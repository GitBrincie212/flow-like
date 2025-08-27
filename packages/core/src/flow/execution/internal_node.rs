use crate::flow::{
    node::{Node, NodeLogic, NodeState},
    pin::PinType,
    utils::evaluate_pin_value,
    variable::VariableType,
};
use ahash::{AHashMap, AHashSet};
use flow_like_types::{Value, json::json, sync::Mutex, utils::ptr_key};
use std::sync::{Arc, Weak, atomic::AtomicU64};

use super::{LogLevel, context::ExecutionContext, internal_pin::InternalPin, log::LogMessage};

#[derive(Debug)]
pub enum InternalNodeError {
    DependencyFailed(String),
    ExecutionFailed(String),
    PinNotReady(String),
}

#[derive(Clone)]
pub struct ExecutionTarget {
    pub node: Arc<InternalNode>,
    pub through_pins: Vec<Arc<Mutex<InternalPin>>>,
}

impl ExecutionTarget {
    async fn into_sub_context(&self, ctx: &mut ExecutionContext) -> ExecutionContext {
        let mut sub = ctx.create_sub_context(&self.node).await;
        sub.started_by = if self.through_pins.is_empty() {
            None
        } else {
            Some(self.through_pins.clone())
        };
        sub
    }
}

async fn exec_deps_from_map(
    ctx: &mut ExecutionContext,
    recursion_guard: &mut Option<AHashSet<String>>,
    dependencies: &AHashMap<String, Vec<Arc<InternalNode>>>,
) -> bool {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Phase {
        Enter,
        Exit,
    }

    let node = ctx.read_node().await;
    let root_id = node.id.clone();

    let mut stack: Vec<(Arc<InternalNode>, Phase)> = Vec::new();
    if let Some(roots) = dependencies.get(&root_id) {
        stack.reserve(roots.len().saturating_mul(2));
        for dep in roots.iter() {
            stack.push((dep.clone(), Phase::Enter));
        }
    }

    let mut scheduled: AHashSet<usize> = AHashSet::with_capacity(stack.len().saturating_mul(2));
    let mut visiting: AHashSet<usize> = AHashSet::with_capacity(stack.len().saturating_mul(2));

    while let Some((n, phase)) = stack.pop() {
        let n_ptr = ptr_key(&n);

        match phase {
            Phase::Enter => {
                if scheduled.contains(&n_ptr) {
                    continue;
                }
                if !visiting.insert(n_ptr) {
                    ctx.log_message(
                        "Cycle detected while resolving mapped dependencies",
                        LogLevel::Error,
                    );
                    return false;
                }
                stack.push((n.clone(), Phase::Exit));

                let dep_id = {
                    let g = n.node.lock().await;
                    g.id.clone()
                };
                if let Some(children) = dependencies.get(&dep_id) {
                    for c in children.iter() {
                        let c_ptr = ptr_key(c);
                        if scheduled.contains(&c_ptr) {
                            continue;
                        }
                        stack.push((c.clone(), Phase::Enter));
                    }
                }
            }
            Phase::Exit => {
                visiting.remove(&n_ptr);
                if scheduled.contains(&n_ptr) {
                    continue;
                }

                // Guard & run the dependency node itself (no successors)
                let (dep_id, dep_name) = {
                    let g = n.node.lock().await;
                    (g.id.clone(), g.friendly_name.clone())
                };

                if let Some(guard) = recursion_guard {
                    if guard.contains(&dep_id) {
                        ctx.log_message(
                            &format!("Recursion detected for: {}, skipping execution", dep_id),
                            LogLevel::Debug,
                        );
                        scheduled.insert(n_ptr);
                        continue;
                    }
                }

                let mut sub = ctx.create_sub_context(&n).await;
                let mut log_message = LogMessage::new(
                    &format!("Triggering mapped dependency: {}", dep_name),
                    LogLevel::Debug,
                    None,
                );

                // Reuse your non-recursive single-node runner
                let res = run_node_logic_only(&mut sub, recursion_guard).await;

                log_message.end();
                ctx.log(log_message);
                sub.end_trace();
                ctx.push_sub_context(&mut sub);

                if res.is_err() {
                    ctx.log_message("Failed to trigger mapped dependency", LogLevel::Error);
                    return false;
                }

                scheduled.insert(n_ptr);
            }
        }
    }

    true
}

async fn run_node_logic_only(
    ctx: &mut ExecutionContext,
    recursion_guard: &mut Option<AHashSet<String>>,
) -> flow_like_types::Result<(), InternalNodeError> {
    ctx.set_state(NodeState::Running).await;
    let node = ctx.read_node().await;

    if recursion_guard.is_none() {
        *recursion_guard = Some(AHashSet::new());
    }
    if let Some(guard) = recursion_guard {
        if guard.contains(&node.id) {
            ctx.log_message(
                &format!("Recursion detected for: {}", &node.id),
                LogLevel::Debug,
            );
            ctx.end_trace();
            return Ok(());
        }
        guard.insert(node.id.clone());
    }

    let logic = ctx.node.logic.clone();
    let mut log_message = LogMessage::new(
        &format!("Starting Node Execution: {} [{}]", &node.name, &node.id),
        LogLevel::Debug,
        None,
    );

    let result = logic.run(ctx).await;

    if let Err(e) = result {
        let err_string = format!("{:?}", e);
        ctx.log_message(
            &format!("Failed to execute node: {}", &err_string),
            LogLevel::Error,
        );
        log_message.end();
        ctx.log(log_message);
        ctx.end_trace();
        ctx.set_state(NodeState::Error).await;
        // NO handle_error() HERE — just bubble up
        return Err(InternalNodeError::ExecutionFailed(node.id));
    }

    ctx.set_state(NodeState::Success).await;
    log_message.end();
    ctx.log(log_message);
    ctx.end_trace();
    Ok(())
}

// --- Helper: collect *pure* parent nodes for a given InternalNode ------------------
async fn pure_parents_for_memo(
    node: &Arc<InternalNode>,
    memo: &mut AHashMap<usize, Vec<Arc<InternalNode>>>,
) -> flow_like_types::Result<Vec<Arc<InternalNode>>> {
    let key = ptr_key(node);
    if let Some(v) = memo.get(&key) {
        return Ok(v.clone());
    }

    let mut result: Vec<Arc<InternalNode>> = Vec::new();
    let pins = node.pins.clone();

    // Iterate only input, non-exec pins. Relay through standalone pins.
    for pin in pins.values() {
        let (is_input, is_exec, depends_on_len, depends_on) = {
            let pin_guard = pin.lock().await;
            let inner_pin = pin_guard.pin.lock().await;
            let is_input = inner_pin.pin_type == PinType::Input;
            let is_exec = inner_pin.data_type == VariableType::Execution;
            let deps_len = pin_guard.depends_on.len();
            (is_input, is_exec, deps_len, pin_guard.depends_on.clone())
        };

        if !is_input || is_exec {
            continue;
        }

        // Pointer-keyed visited for pins; reserve generously to avoid rehash
        let mut visited_pins: AHashSet<usize> =
            AHashSet::with_capacity(depends_on_len.saturating_mul(4));
        let mut stack: Vec<Weak<Mutex<InternalPin>>> = depends_on;

        while let Some(dep_weak) = stack.pop() {
            let Some(dep_arc) = dep_weak.upgrade() else {
                continue;
            };
            let pin_key = ptr_key(&dep_arc);
            if !visited_pins.insert(pin_key) {
                continue;
            }

            let parent_opt = {
                let dep_guard = dep_arc.lock().await;
                if let Some(node_weak) = &dep_guard.node {
                    node_weak.upgrade()
                } else {
                    // standalone/relay pin => follow further upstream
                    if !dep_guard.depends_on.is_empty() {
                        stack.extend(dep_guard.depends_on.iter().cloned());
                    }
                    None
                }
            };

            if let Some(parent) = parent_opt {
                if parent.is_pure().await {
                    result.push(parent);
                }
            }
        }
    }

    // (Optional) de-dup parents by pointer to avoid re-executing same pure node.
    if result.len() > 1 {
        let mut seen: AHashSet<usize> = AHashSet::with_capacity(result.len());
        result.retain(|n| seen.insert(ptr_key(n)));
    }

    memo.insert(key, result.clone());
    Ok(result)
}

pub struct InternalNode {
    pub node: Arc<Mutex<Node>>,
    pub pins: AHashMap<String, Arc<Mutex<InternalPin>>>,
    pub logic: Arc<dyn NodeLogic>,
    pub exec_calls: AtomicU64,
    pin_name_cache: Mutex<AHashMap<String, Vec<Arc<Mutex<InternalPin>>>>>,
}

impl InternalNode {
    pub fn new(
        node: Node,
        pins: AHashMap<String, Arc<Mutex<InternalPin>>>,
        logic: Arc<dyn NodeLogic>,
        name_cache: AHashMap<String, Vec<Arc<Mutex<InternalPin>>>>,
    ) -> Self {
        InternalNode {
            node: Arc::new(Mutex::new(node)),
            pins,
            logic,
            pin_name_cache: Mutex::new(name_cache),
            exec_calls: AtomicU64::new(0),
        }
    }

    pub async fn ensure_cache(&self, name: &str) {
        {
            let cache = self.pin_name_cache.lock().await;
            if cache.contains_key(name) {
                return;
            }
        }

        let mut pins_by_name = AHashMap::new();
        for pin_ref in self.pins.values() {
            let pin_name = {
                let pin_guard = pin_ref.lock().await;
                let pin = pin_guard.pin.lock().await;
                pin.name.clone()
            };

            pins_by_name
                .entry(pin_name)
                .or_insert_with(Vec::new)
                .push(pin_ref.clone());
        }

        let mut cache = self.pin_name_cache.lock().await;
        for (pin_name, pins) in pins_by_name {
            cache.entry(pin_name).or_insert(pins);
        }
    }

    pub async fn get_pin_by_name(
        &self,
        name: &str,
    ) -> flow_like_types::Result<Arc<Mutex<InternalPin>>> {
        self.ensure_cache(name).await;

        let pin = {
            let cache = self.pin_name_cache.lock().await;
            cache
                .get(name)
                .and_then(|pins_ref| pins_ref.first().cloned())
        };

        let pin = pin.ok_or(flow_like_types::anyhow!("Pin {} not found", name))?;
        Ok(pin)
    }

    pub async fn get_pins_by_name(
        &self,
        name: &str,
    ) -> flow_like_types::Result<Vec<Arc<Mutex<InternalPin>>>> {
        self.ensure_cache(name).await;
        let cache = self.pin_name_cache.lock().await;
        if let Some(pins_ref) = cache.get(name) {
            return Ok(pins_ref.clone());
        }

        Err(flow_like_types::anyhow!("Pin {} not found", name))
    }

    pub fn get_pin_by_id(&self, id: &str) -> flow_like_types::Result<Arc<Mutex<InternalPin>>> {
        if let Some(pin) = self.pins.get(id) {
            return Ok(pin.clone());
        }

        Err(flow_like_types::anyhow!("Pin {} not found", id))
    }

    pub async fn orphaned(&self) -> bool {
        for pin in self.pins.values() {
            let pin_guard = pin.lock().await.pin.clone();
            let pin = pin_guard.lock().await;

            if pin.pin_type != PinType::Input {
                continue;
            }

            if pin.depends_on.is_empty() && pin.default_value.is_none() {
                return true;
            }
        }

        false
    }

    pub async fn is_ready(&self) -> flow_like_types::Result<bool> {
        for pin in self.pins.values() {
            let pin_guard = pin.lock().await;
            let pin = pin_guard.pin.lock().await;

            if pin.pin_type != PinType::Input {
                continue;
            }

            if pin.depends_on.is_empty() && pin.default_value.is_none() {
                return Ok(false);
            }

            // execution pins can have multiple inputs for different paths leading to it. We only need to make sure that one of them is valid!
            let is_execution = pin.data_type == VariableType::Execution;
            let mut execution_valid = false;
            let depends_on = pin_guard.depends_on.clone();
            drop(pin);
            drop(pin_guard);

            for depends_on_pin in depends_on {
                let depends_on_pin = depends_on_pin
                    .upgrade()
                    .ok_or(flow_like_types::anyhow!("Failed to lock Pin"))?;
                let depends_on_pin_guard = depends_on_pin.lock().await;
                let depends_on_pin = depends_on_pin_guard.pin.lock().await;

                // non execution pins need all inputs to be valid
                if depends_on_pin.value.is_none() && !is_execution {
                    return Ok(false);
                }

                if depends_on_pin.value.is_some() {
                    execution_valid = true;
                }
            }

            if is_execution && !execution_valid {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub async fn get_connected(&self) -> flow_like_types::Result<Vec<Arc<InternalNode>>> {
        let mut connected = Vec::with_capacity(self.pins.len());
        let mut seen_nodes: AHashSet<usize> = AHashSet::new();
        let mut visited_pins: AHashSet<usize> = AHashSet::new();
        let mut stack: Vec<Weak<Mutex<InternalPin>>> = Vec::new();

        for pin in self.pins.values() {
            let pin_guard = pin.lock().await;
            let pin = pin_guard.pin.lock().await;

            if pin.pin_type != PinType::Output {
                continue;
            }
            drop(pin);

            let seeds = pin_guard.connected_to.clone();
            drop(pin_guard);

            let cap = seeds.len();
            visited_pins.clear();
            stack.clear();
            if stack.capacity() < cap {
                stack.reserve(cap - stack.capacity());
            }
            stack.extend(seeds);

            while let Some(next_weak) = stack.pop() {
                let pin_arc = next_weak
                    .upgrade()
                    .ok_or(flow_like_types::anyhow!("Failed to lock Pin"))?;

                let pin_key = Arc::as_ptr(&pin_arc) as usize;
                if !visited_pins.insert(pin_key) {
                    continue;
                }

                let parent_opt = {
                    let guard = pin_arc.lock().await;
                    if let Some(node_weak) = &guard.node {
                        node_weak.upgrade()
                    } else {
                        stack.extend(guard.connected_to.iter().cloned());
                        None
                    }
                };

                if let Some(parent) = parent_opt {
                    let node_key = Arc::as_ptr(&parent) as usize;
                    if seen_nodes.insert(node_key) {
                        connected.push(parent);
                    }
                }
            }
        }

        Ok(connected)
    }

    pub async fn get_connected_exec(
        &self,
        filter_valid: bool,
    ) -> flow_like_types::Result<Vec<ExecutionTarget>> {
        // node_ptr -> (node_arc, pins_vec, seen_pin_ptrs)
        let mut groups: AHashMap<
            usize,
            (
                Arc<InternalNode>,
                Vec<Arc<Mutex<InternalPin>>>,
                AHashSet<usize>,
            ),
        > = AHashMap::with_capacity(16);

        let mut visited_pins: AHashSet<usize> = AHashSet::with_capacity(64);
        let mut stack: Vec<Weak<Mutex<InternalPin>>> = Vec::with_capacity(64);

        for pin in self.pins.values() {
            // Only consider exec OUTPUTs; evaluate filter after that
            let pin_g = pin.lock().await;
            let meta = pin_g.pin.lock().await;
            if meta.pin_type != PinType::Output || meta.data_type != VariableType::Execution {
                continue;
            }
            drop(meta);

            if filter_valid {
                match evaluate_pin_value(pin.clone()).await {
                    Ok(Value::Bool(true)) => {}
                    _ => continue,
                }
            }

            let seeds = pin_g.connected_to.clone();
            drop(pin_g);

            visited_pins.clear();
            stack.clear();
            stack.extend(seeds);

            while let Some(next_weak) = stack.pop() {
                let Some(pin_arc) = next_weak.upgrade() else {
                    continue;
                };
                let pkey = ptr_key(&pin_arc);
                if !visited_pins.insert(pkey) {
                    continue;
                }

                let parent_opt = {
                    let g = pin_arc.lock().await;
                    if let Some(node_w) = &g.node {
                        node_w.upgrade()
                    } else {
                        // relay pin; keep walking
                        stack.extend(g.connected_to.iter().cloned());
                        None
                    }
                };

                if let Some(parent) = parent_opt {
                    let nkey = ptr_key(&parent);
                    let entry = groups.entry(nkey).or_insert_with(|| {
                        (
                            parent.clone(),
                            Vec::with_capacity(2),
                            AHashSet::with_capacity(4),
                        )
                    });
                    // dedup pin within the node group
                    if entry.2.insert(pkey) {
                        entry.1.push(pin_arc.clone());
                    }
                }
            }
        }

        // materialize
        let mut out = Vec::with_capacity(groups.len());
        for (_, (node, pins, _seen)) in groups {
            out.push(ExecutionTarget {
                node,
                through_pins: pins,
            });
        }
        Ok(out)
    }

    pub async fn get_error_handled_nodes(&self) -> flow_like_types::Result<Vec<Arc<InternalNode>>> {
        let pin = self.get_pin_by_name("auto_handle_error").await?;
        let active = evaluate_pin_value(pin.clone()).await?;
        let active = match active {
            Value::Bool(b) => b,
            _ => false,
        };
        if !active {
            return Err(flow_like_types::anyhow!("Error Pin not active"));
        }

        let pin_guard = pin.lock().await;
        let pin_meta = pin_guard.pin.lock().await;
        if pin_meta.pin_type != PinType::Output {
            return Err(flow_like_types::anyhow!("Pin is not an output pin"));
        }
        if pin_meta.data_type != VariableType::Execution {
            return Err(flow_like_types::anyhow!("Pin is not an execution pin"));
        }
        drop(pin_meta);

        let seeds = pin_guard.connected_to.clone();
        drop(pin_guard);

        let cap = seeds.len();
        let mut connected = Vec::with_capacity(cap);
        let mut seen_nodes: AHashSet<usize> = AHashSet::with_capacity(cap.saturating_mul(2));
        let mut visited_pins: AHashSet<usize> = AHashSet::with_capacity(cap.saturating_mul(4));
        let mut stack: Vec<Weak<Mutex<InternalPin>>> = seeds;

        while let Some(next_weak) = stack.pop() {
            let pin_arc = next_weak
                .upgrade()
                .ok_or(flow_like_types::anyhow!("Failed to lock Pin"))?;

            let pin_key = Arc::as_ptr(&pin_arc) as usize;
            if !visited_pins.insert(pin_key) {
                continue;
            }

            let parent_opt = {
                let guard = pin_arc.lock().await;
                if let Some(node_weak) = &guard.node {
                    node_weak.upgrade()
                } else {
                    // relay through standalone pins
                    stack.extend(guard.connected_to.iter().cloned());
                    None
                }
            };

            if let Some(parent) = parent_opt {
                let node_key = Arc::as_ptr(&parent) as usize;
                if seen_nodes.insert(node_key) {
                    connected.push(parent);
                }
            }
        }

        Ok(connected)
    }

    pub async fn get_dependencies(&self) -> flow_like_types::Result<Vec<Arc<InternalNode>>> {
        let mut dependencies = Vec::with_capacity(self.pins.len());
        let mut seen_nodes: AHashSet<usize> = AHashSet::new();
        let mut visited_pins: AHashSet<usize> = AHashSet::new();
        let mut stack: Vec<Weak<Mutex<InternalPin>>> = Vec::new();

        for pin in self.pins.values() {
            let pin_guard = pin.lock().await;
            let pin_meta = pin_guard.pin.lock().await;

            if pin_meta.pin_type != PinType::Input {
                continue;
            }
            drop(pin_meta);

            let seeds = pin_guard.depends_on.clone();
            drop(pin_guard);

            let cap = seeds.len();
            visited_pins.clear();
            stack.clear();
            if stack.capacity() < cap {
                stack.reserve(cap - stack.capacity());
            }
            stack.extend(seeds);

            while let Some(dep_weak) = stack.pop() {
                let dep_arc = dep_weak
                    .upgrade()
                    .ok_or(flow_like_types::anyhow!("Failed to lock Pin"))?;

                let pin_key = Arc::as_ptr(&dep_arc) as usize;
                if !visited_pins.insert(pin_key) {
                    continue;
                }

                let parent_opt = {
                    let dep_guard = dep_arc.lock().await;
                    if let Some(node_weak) = &dep_guard.node {
                        node_weak.upgrade()
                    } else {
                        stack.extend(dep_guard.depends_on.iter().cloned());
                        None
                    }
                };

                if let Some(parent) = parent_opt {
                    let node_key = Arc::as_ptr(&parent) as usize;
                    if seen_nodes.insert(node_key) {
                        dependencies.push(parent);
                    }
                }
            }
        }

        Ok(dependencies)
    }

    pub async fn is_pure(&self) -> bool {
        let node = self.node.lock().await;
        let pins = node
            .pins
            .values()
            .find(|pin| pin.data_type == VariableType::Execution);
        pins.is_none()
    }

    pub async fn trigger_missing_dependencies(
        context: &mut ExecutionContext,
        recursion_guard: &mut Option<AHashSet<String>>,
        _with_successors: bool, // not used here
    ) -> bool {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Phase {
            Enter,
            Exit,
        }

        let mut parents_memo: AHashMap<usize, Vec<Arc<InternalNode>>> = AHashMap::with_capacity(16);

        // Seed: pure parents of the current node. Dedup by pointer.
        let mut roots = match pure_parents_for_memo(&context.node, &mut parents_memo).await {
            Ok(v) => v,
            Err(_) => {
                context.log_message("Failed to collect dependencies", LogLevel::Error);
                return false;
            }
        };
        if roots.len() > 1 {
            let mut seen: AHashSet<usize> = AHashSet::with_capacity(roots.len());
            roots.retain(|n| seen.insert(ptr_key(n)));
        }

        let mut stack: Vec<(Arc<InternalNode>, Phase)> =
            Vec::with_capacity(roots.len().saturating_mul(2));
        for n in roots {
            stack.push((n, Phase::Enter));
        }

        let mut scheduled: AHashSet<usize> = AHashSet::with_capacity(stack.len().saturating_mul(2));
        let mut visiting: AHashSet<usize> = AHashSet::with_capacity(stack.len().saturating_mul(2));

        while let Some((node_arc, phase)) = stack.pop() {
            let node_ptr = ptr_key(&node_arc);

            match phase {
                Phase::Enter => {
                    if scheduled.contains(&node_ptr) {
                        continue;
                    }
                    if !visiting.insert(node_ptr) {
                        context.log_message(
                            "Cycle detected while resolving dependencies",
                            LogLevel::Error,
                        );
                        return false;
                    }

                    // Post-order: revisit on Exit
                    stack.push((node_arc.clone(), Phase::Exit));

                    // Push this node's pure parents first (dedup against scheduled for cheap)
                    match pure_parents_for_memo(&node_arc, &mut parents_memo).await {
                        Ok(parents) => {
                            // Iterate parents in natural order; for more cache locality you could reverse.
                            for p in parents {
                                let p_ptr = ptr_key(&p);
                                if scheduled.contains(&p_ptr) {
                                    continue;
                                }
                                stack.push((p, Phase::Enter));
                            }
                        }
                        Err(e) => {
                            context.log_message(
                                &format!("Failed to collect parents: {:?}", e),
                                LogLevel::Error,
                            );
                            return false;
                        }
                    }
                }
                Phase::Exit => {
                    visiting.remove(&node_ptr);
                    if scheduled.contains(&node_ptr) {
                        continue;
                    }

                    // Get id/name once (short lock scope).
                    let (node_id, node_name) = {
                        let parent_node = node_arc.node.lock().await;
                        (parent_node.id.clone(), parent_node.friendly_name.clone())
                    };

                    if let Some(guard) = recursion_guard {
                        if guard.contains(&node_id) {
                            context.log_message(
                                &format!(
                                    "Recursion detected for: {}, skipping execution",
                                    &node_id
                                ),
                                LogLevel::Debug,
                            );
                            scheduled.insert(node_ptr);
                            continue;
                        }
                    }

                    // Execute dependency (no successors)
                    let mut sub = context.create_sub_context(&node_arc).await;
                    let mut log_message = LogMessage::new(
                        &format!("Triggering missing dependency: {}", &node_name),
                        LogLevel::Debug,
                        None,
                    );
                    let res = run_node_logic_only(&mut sub, recursion_guard).await;
                    log_message.end();
                    context.log(log_message);
                    sub.end_trace();
                    context.push_sub_context(&mut sub);

                    if res.is_err() {
                        context.log_message(
                            &format!("Failed to trigger dependency: {}", &node_name),
                            LogLevel::Error,
                        );
                        return false;
                    }

                    scheduled.insert(node_ptr);
                }
            }
        }

        true
    }

    pub async fn handle_error(
        context: &mut ExecutionContext,
        error: &str,
        recursion_guard: &mut Option<AHashSet<String>>,
    ) -> Result<(), InternalNodeError> {
        let _ = context.activate_exec_pin("auto_handle_error").await;
        let _ = context
            .set_pin_value("auto_handle_error_string", json!(error))
            .await;

        let connected = context
            .node
            .get_error_handled_nodes()
            .await
            .map_err(|err| {
                context.log_message(
                    &format!("Failed to get error handling nodes: {}", err),
                    LogLevel::Error,
                );
                InternalNodeError::ExecutionFailed(context.id.clone())
            })?;

        if connected.is_empty() {
            context.log_message(
                &format!("No error handling nodes found for: {}", &context.id),
                LogLevel::Error,
            );
            return Err(InternalNodeError::ExecutionFailed(context.id.clone()));
        }

        // Iterate each error handler and walk its successors iteratively (DFS).
        for handler in connected {
            let mut sub = context.create_sub_context(&handler).await;

            // Use SAME recursion_guard here (parity with original)
            if !InternalNode::trigger_missing_dependencies(&mut sub, recursion_guard, false).await {
                let err_string =
                    "Failed to trigger missing dependencies for error handler".to_string();
                let _ = sub
                    .set_pin_value("auto_handle_error_string", json!(err_string))
                    .await;
                sub.end_trace();
                context.push_sub_context(&mut sub);
                return Err(InternalNodeError::ExecutionFailed(context.id.clone()));
            }

            // run handler node
            if let Err(e) = run_node_logic_only(&mut sub, recursion_guard).await {
                let err_string = format!("{:?}", e);
                let _ = sub
                    .set_pin_value("auto_handle_error_string", json!(err_string))
                    .await;
                sub.end_trace();
                context.push_sub_context(&mut sub);
                return Err(InternalNodeError::ExecutionFailed(context.id.clone()));
            }

            // walk successors of the error handler (still using the same guard)
            let mut stack: Vec<ExecutionTarget> = match handler.get_connected_exec(true).await {
                Ok(v) => v,
                Err(err) => {
                    let err_string = format!("{:?}", err);
                    let _ = sub
                        .set_pin_value("auto_handle_error_string", json!(err_string))
                        .await;
                    sub.end_trace();
                    context.push_sub_context(&mut sub);
                    return Err(InternalNodeError::ExecutionFailed(context.id.clone()));
                }
            };

            let mut seen_exec_ptrs: ahash::AHashSet<usize> =
                ahash::AHashSet::with_capacity(stack.len().saturating_mul(2));

            while let Some(next) = stack.pop() {
                let key = Arc::as_ptr(&next.node) as usize;
                if !seen_exec_ptrs.insert(key) {
                    continue;
                }

                let mut sub2 = next.into_sub_context(context).await;

                if !InternalNode::trigger_missing_dependencies(&mut sub2, recursion_guard, false)
                    .await
                {
                    let err_string =
                        "Failed to trigger successor dependencies (error chain)".to_string();
                    let _ = sub2
                        .set_pin_value("auto_handle_error_string", json!(err_string))
                        .await;
                    sub2.end_trace();
                    context.push_sub_context(&mut sub2);
                    let _ = sub
                        .set_pin_value("auto_handle_error_string", json!("error chain aborted"))
                        .await;
                    sub.end_trace();
                    context.push_sub_context(&mut sub);
                    return Err(InternalNodeError::ExecutionFailed(context.id.clone()));
                }

                if let Err(e) = run_node_logic_only(&mut sub2, recursion_guard).await {
                    let err_string = format!("{:?}", e);
                    let _ = sub2
                        .set_pin_value("auto_handle_error_string", json!(err_string))
                        .await;
                    sub2.end_trace();
                    context.push_sub_context(&mut sub2);
                    let _ = sub
                        .set_pin_value("auto_handle_error_string", json!("error chain aborted"))
                        .await;
                    sub.end_trace();
                    context.push_sub_context(&mut sub);
                    return Err(InternalNodeError::ExecutionFailed(context.id.clone()));
                }

                match next.node.get_connected_exec(true).await {
                    Ok(more) => {
                        for s in more {
                            stack.push(s);
                        }
                    }
                    Err(err) => {
                        let err_string = format!("{:?}", err);
                        let _ = sub2
                            .set_pin_value("auto_handle_error_string", json!(err_string))
                            .await;
                        sub2.end_trace();
                        context.push_sub_context(&mut sub2);
                        let _ = sub
                            .set_pin_value("auto_handle_error_string", json!("error chain aborted"))
                            .await;
                        sub.end_trace();
                        context.push_sub_context(&mut sub);
                        return Err(InternalNodeError::ExecutionFailed(context.id.clone()));
                    }
                }

                sub2.end_trace();
                context.push_sub_context(&mut sub2);
            }

            sub.end_trace();
            context.push_sub_context(&mut sub);
        }

        context.set_state(NodeState::Error).await;
        Ok(())
    }

    pub async fn trigger(
        context: &mut ExecutionContext,
        recursion_guard: &mut Option<AHashSet<String>>,
        with_successors: bool,
    ) -> flow_like_types::Result<(), InternalNodeError> {
        // deps
        if !InternalNode::trigger_missing_dependencies(context, recursion_guard, false).await {
            context.log_message("Failed to trigger missing dependencies", LogLevel::Error);
            context.end_trace();
            InternalNode::handle_error(
                context,
                "Failed to trigger missing dependencies",
                recursion_guard,
            )
            .await?;
            let node = context.read_node().await;
            return Err(InternalNodeError::DependencyFailed(node.id));
        }

        // this node
        if let Err(e) = run_node_logic_only(context, recursion_guard).await {
            let err_string = format!("{:?}", e);
            InternalNode::handle_error(context, &err_string, recursion_guard).await?;
            let node = context.read_node().await;
            return Err(InternalNodeError::ExecutionFailed(node.id));
        }

        // successors (DFS; fresh guard per successor to mirror old semantics)
        if with_successors {
            let successors = match context.node.get_connected_exec(true).await {
                Ok(nodes) => nodes,
                Err(err) => {
                    let err_string = format!("{:?}", err);
                    context.log_message(
                        &format!("Failed to get successors: {}", err_string),
                        LogLevel::Error,
                    );
                    InternalNode::handle_error(context, &err_string, recursion_guard).await?;
                    let node = context.read_node().await;
                    return Err(InternalNodeError::ExecutionFailed(node.id));
                }
            };

            let mut stack: Vec<ExecutionTarget> = Vec::with_capacity(successors.len());
            stack.extend(successors);

            let mut seen_exec_ptrs: ahash::AHashSet<usize> =
                ahash::AHashSet::with_capacity(stack.len().saturating_mul(2));

            while let Some(next) = stack.pop() {
                let key = Arc::as_ptr(&next.node) as usize;
                if !seen_exec_ptrs.insert(key) {
                    continue;
                }

                let mut sub = next.into_sub_context(context).await;
                let mut local_guard: Option<AHashSet<String>> = None;

                if !InternalNode::trigger_missing_dependencies(&mut sub, &mut local_guard, false)
                    .await
                {
                    let err_string = "Failed to trigger successor dependencies".to_string();
                    InternalNode::handle_error(&mut sub, &err_string, &mut local_guard).await?;
                    sub.end_trace();
                    context.push_sub_context(&mut sub);
                    let node = context.read_node().await;
                    return Err(InternalNodeError::ExecutionFailed(node.id));
                }

                if let Err(e) = run_node_logic_only(&mut sub, &mut local_guard).await {
                    let err_string = format!("{:?}", e);
                    let _ = sub.activate_exec_pin("auto_handle_error").await;
                    let _ = sub
                        .set_pin_value("auto_handle_error_string", json!(err_string))
                        .await;
                    sub.end_trace();
                    context.push_sub_context(&mut sub);
                    let node = context.read_node().await;
                    return Err(InternalNodeError::ExecutionFailed(node.id));
                }

                match next.node.get_connected_exec(true).await {
                    Ok(more) => {
                        for s in more {
                            stack.push(s);
                        }
                    }
                    Err(err) => {
                        let err_string = format!("{:?}", err);
                        InternalNode::handle_error(&mut sub, &err_string, &mut local_guard).await?;
                        sub.end_trace();
                        context.push_sub_context(&mut sub);
                        let node = context.read_node().await;
                        return Err(InternalNodeError::ExecutionFailed(node.id));
                    }
                }

                sub.end_trace();
                context.push_sub_context(&mut sub);
            }
        }

        Ok(())
    }

    pub async fn trigger_with_dependencies(
        context: &mut ExecutionContext,
        recursion_guard: &mut Option<AHashSet<String>>,
        with_successors: bool,
        dependencies: &AHashMap<String, Vec<Arc<InternalNode>>>,
    ) -> flow_like_types::Result<(), InternalNodeError> {
        context.set_state(NodeState::Running).await;

        let node = context.read_node().await;

        if recursion_guard.is_none() {
            *recursion_guard = Some(AHashSet::new());
        }
        if let Some(guard) = recursion_guard {
            if guard.contains(&node.id) {
                context.log_message(
                    &format!("Recursion detected for: {}", &node.id),
                    LogLevel::Debug,
                );
                context.end_trace();
                return Ok(());
            }
            guard.insert(node.id.clone());
        }

        // 1) Execute precomputed dependencies iteratively (no recursion)
        if !exec_deps_from_map(context, recursion_guard, dependencies).await {
            let err = "Failed to trigger mapped dependencies".to_string();
            InternalNode::handle_error(context, &err, recursion_guard).await?;
            return Err(InternalNodeError::DependencyFailed(node.id.clone()));
        }

        // 2) Run this node (no successors here)
        let logic = context.node.logic.clone();
        let mut log_message = LogMessage::new(
            &format!("Starting Node Execution: {} [{}]", &node.name, &node.id),
            LogLevel::Debug,
            None,
        );
        let result = logic.run(context).await;

        if let Err(e) = result {
            let err_string = format!("{:?}", e);
            context.log_message(
                &format!("Failed to execute node: {}", err_string),
                LogLevel::Error,
            );
            log_message.end();
            context.log(log_message);
            context.end_trace();
            context.set_state(NodeState::Error).await;
            InternalNode::handle_error(context, &err_string, recursion_guard).await?;
            return Err(InternalNodeError::ExecutionFailed(node.id.clone()));
        }

        context.set_state(NodeState::Success).await;
        log_message.end();
        context.log(log_message);
        context.end_trace();

        // 3) Walk successors iteratively (DFS), like your non-recursive `trigger`
        if with_successors {
            let successors = match context.node.get_connected_exec(true).await {
                Ok(nodes) => nodes,
                Err(err) => {
                    let err_string = format!("{:?}", err);
                    context.log_message(
                        &format!("Failed to get successors: {}", err_string.clone()),
                        LogLevel::Error,
                    );
                    InternalNode::handle_error(context, &err_string, recursion_guard).await?;
                    return Err(InternalNodeError::ExecutionFailed(node.id.clone()));
                }
            };

            let mut stack: Vec<ExecutionTarget> = Vec::with_capacity(successors.len());
            stack.extend(successors);

            let mut seen_exec_ptrs: ahash::AHashSet<usize> =
                ahash::AHashSet::with_capacity(stack.len().saturating_mul(2));

            while let Some(next) = stack.pop() {
                let key = Arc::as_ptr(&next.node) as usize;
                if !seen_exec_ptrs.insert(key) {
                    continue;
                }

                let mut sub = next.into_sub_context(context).await;

                // Fresh recursion guard per successor to mirror original semantics
                let mut local_guard: Option<AHashSet<String>> = None;

                // Execute *its* mapped deps (fresh executed set semantics like before)
                if !exec_deps_from_map(&mut sub, &mut local_guard, dependencies).await {
                    let err_string = "Failed to trigger successor mapped dependencies".to_string();
                    InternalNode::handle_error(&mut sub, &err_string, &mut local_guard).await?;
                    sub.end_trace();
                    context.push_sub_context(&mut sub);
                    return Err(InternalNodeError::ExecutionFailed(node.id.clone()));
                }

                // Run successor node
                if let Err(e) = run_node_logic_only(&mut sub, &mut local_guard).await {
                    let err_string = format!("{:?}", e);
                    let _ = sub.activate_exec_pin("auto_handle_error").await;
                    let _ = sub
                        .set_pin_value("auto_handle_error_string", json!(err_string))
                        .await;
                    sub.end_trace();
                    context.push_sub_context(&mut sub);
                    return Err(InternalNodeError::ExecutionFailed(node.id.clone()));
                }

                // Enqueue its successors (DFS)
                match next.node.get_connected_exec(true).await {
                    Ok(more) => {
                        for s in more {
                            stack.push(s);
                        }
                    }
                    Err(err) => {
                        let err_string = format!("{:?}", err);
                        InternalNode::handle_error(&mut sub, &err_string, &mut local_guard).await?;
                        sub.end_trace();
                        context.push_sub_context(&mut sub);
                        return Err(InternalNodeError::ExecutionFailed(node.id.clone()));
                    }
                }

                sub.end_trace();
                context.push_sub_context(&mut sub);
            }
        }

        Ok(())
    }
}
