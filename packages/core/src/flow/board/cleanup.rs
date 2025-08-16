use std::{collections::HashMap, sync::Arc};

use crate::flow::{board::{cleanup::{bridge_layers::BridgeLayersCleanup, fix_pin_connections::FixPinsCleanup, fix_refs::FixRefsCleanup, order_pin_indices::PinIndicesCleanup}, Board, Layer}, node::Node, pin::Pin};

pub mod bridge_layers;
pub mod fix_pin_connections;
pub mod fix_refs;
pub mod fix_initial_coordinates;
pub mod order_pin_indices;

pub type PinLookup = HashMap<String, (Arc<Pin>, NodeOrLayer)>;

pub enum NodeOrLayer {
    Node(String),
    Layer(String),
}

pub enum NodeOrLayerRef<'a> {
    Node(&'a Node),
    Layer(&'a Layer),
}

impl NodeOrLayerRef<'_> {
    pub fn id(&self) -> &str {
        match self {
            NodeOrLayerRef::Node(node) => node.id.as_str(),
            NodeOrLayerRef::Layer(layer) => layer.id.as_str(),
        }
    }
}

impl NodeOrLayer {
    pub fn is_node(&self) -> bool {
        matches!(self, NodeOrLayer::Node(_))
    }

    pub fn is_layer(&self) -> bool {
        matches!(self, NodeOrLayer::Layer(_))
    }

    pub fn id(&self) -> &str {
        match self {
            NodeOrLayer::Node(node_id) => node_id.as_str(),
            NodeOrLayer::Layer(layer_id) => layer_id.as_str(),
        }
    }
}

pub trait BoardCleanupLogic {
    fn init(board: &mut Board) -> Self
    where
        Self: Sized;

    fn initial_node_iteration(&mut self, _node: &Node) {}
    fn main_node_iteration(&mut self, _node: &mut Node, _pin_lookup: &PinLookup) {}

    fn initial_pin_iteration(&mut self, _pin: &Pin, _parent: NodeOrLayerRef) {}
    fn main_pin_iteration(&mut self, _pin: &mut Pin, _pin_lookup: &PinLookup) {}

    fn initial_layer_iteration(&mut self, _layer: &Layer) {}
    fn main_layer_iteration(&mut self, _layer: &mut Layer, _pin_lookup: &PinLookup) {}

    fn post_process(&mut self, _board: &mut Board, _pin_lookup: &PinLookup) {}
}

impl Board {
    pub fn cleanup(&mut self) {
        let mut bridge_layers = BridgeLayersCleanup::init(self);
        let mut fix_pin_connections = FixPinsCleanup::init(self);
        let mut fix_refs = FixRefsCleanup::init(self);
        let mut order_pin_indices = PinIndicesCleanup::init(self);
        let mut fix_initial_coordinates = fix_initial_coordinates::FixInitialCoordinates::init(self);

        let mut pins: PinLookup = HashMap::with_capacity((self.nodes.len() + self.layers.len()) * 4);

        let mut steps: Vec<&mut dyn BoardCleanupLogic> = vec![
            &mut fix_initial_coordinates,
            &mut fix_refs,
            &mut fix_pin_connections,
            &mut bridge_layers,
            &mut order_pin_indices,
        ];

        // Initial Processing
        for node in self.nodes.values() {
            for step in steps.iter_mut() {
                (*step).initial_node_iteration(node);
            }

            for pin in node.pins.values() {
                pins.insert(pin.id.clone(), (Arc::new(pin.clone()), NodeOrLayer::Node(node.id.clone())));
                for step in steps.iter_mut() {
                    (*step).initial_pin_iteration(pin, NodeOrLayerRef::Node(node));
                }
            }
        }

        for layer in self.layers.values() {
            for step in steps.iter_mut() {
                (*step).initial_layer_iteration(layer);
            }

            for pin in layer.pins.values() {
                pins.insert(pin.id.clone(), (Arc::new(pin.clone()), NodeOrLayer::Layer(layer.id.clone())));
                for step in steps.iter_mut() {
                    (*step).initial_pin_iteration(pin, NodeOrLayerRef::Layer(layer));
                }
            }
        }

        // Main Processing
        for node in self.nodes.values_mut() {
            for step in steps.iter_mut() {
                (*step).main_node_iteration(node, &pins);
            }

            for pin in node.pins.values_mut() {
                for step in steps.iter_mut() {
                    (*step).main_pin_iteration(pin, &pins);
                }
            }
        }

        for layer in self.layers.values_mut() {
            for step in steps.iter_mut() {
                (*step).main_layer_iteration(layer, &pins);
            }

            for pin in layer.pins.values_mut() {
                for step in steps.iter_mut() {
                    (*step).main_pin_iteration(pin, &pins);
                }
            }
        }

        for step in steps.iter_mut() {
            (*step).post_process(self, &pins);
        }
    }
}