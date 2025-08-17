use crate::{image::NodeImage, storage::path::FlowPath};
use flow_like::{
    flow::{
        execution::context::ExecutionContext,
        node::{Node, NodeLogic},
        pin::{PinOptions, ValueType},
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{
    JsonSchema, Result, async_trait,
    image::{imageops::FilterType, RgbImage, imageops},
    json::{Deserialize, Serialize, json},
    tokio,
};
use std::io::Cursor;
use tract_tflite::prelude::*;

#[derive(Default, Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct ClassPrediction {
    pub class_idx: u32,
    pub score: f32,
    pub label: Option<String>,
}

#[derive(Default)]
pub struct TeachableMachineNode {}

impl TeachableMachineNode {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl NodeLogic for TeachableMachineNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "ai_ml_teachable_machine",
            "Teachable Machine",
            "Image classification using Teachable Machine models.",
            "AI/ML",
        );

        node.add_icon("/flow/icons/find_model.svg");

        node.add_input_pin(
            "exec_in",
            "Input",
            "Initiate Execution",
            VariableType::Execution,
        );

        node.add_input_pin(
            "model",
            "Model File",
            "Path to *.tflite model",
            VariableType::Struct,
        )
        .set_schema::<FlowPath>()
        .set_options(PinOptions::new().set_enforce_schema(true).build());

        node.add_input_pin("image_in", "Image", "Image Object", VariableType::Struct)
            .set_schema::<NodeImage>()
            .set_options(PinOptions::new().set_enforce_schema(true).build());

        node.add_input_pin(
            "labels",
            "Labels",
            "Optional labels.txt",
            VariableType::Struct,
        )
        .set_schema::<FlowPath>()
        .set_options(PinOptions::new().set_enforce_schema(true).build());

        node.add_input_pin(
            "input_width",
            "Input Width",
            "Model input width",
            VariableType::Integer,
        )
        .set_default_value(Some(json!(224)));
        node.add_input_pin(
            "input_height",
            "Input Height",
            "Model input height",
            VariableType::Integer,
        )
        .set_default_value(Some(json!(224)));

        node.add_output_pin(
            "exec_out",
            "Output",
            "Done with the Execution",
            VariableType::Execution,
        );
        node.add_output_pin(
            "predictions",
            "Predictions",
            "Class Predictions",
            VariableType::Struct,
        )
        .set_value_type(ValueType::Array);

        node
    }

    async fn run(&self, context: &mut ExecutionContext) -> Result<()> {
        context.deactivate_exec_pin("exec_out").await?;

        let model_path: FlowPath = context.evaluate_pin("model").await?;
        let node_img: NodeImage = context.evaluate_pin("image_in").await?;
        let labels_path_opt: Option<FlowPath> = context.evaluate_pin("labels").await.ok();
        let iw_pin: i64 = context.evaluate_pin("input_width").await.unwrap_or(224);
        let ih_pin: i64 = context.evaluate_pin("input_height").await.unwrap_or(224);
        let (iw, ih) = (iw_pin.max(1) as u32, ih_pin.max(1) as u32);

        let labels_vec: Vec<String> = if let Some(labels_path) = labels_path_opt {
            let lbs = labels_path.get(context, false).await?;
            let s = String::from_utf8(lbs)
                .map_err(|e| flow_like_types::anyhow!("Failed to parse labels file: {}", e))?;
            s.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        let raw = model_path.get(context, false).await.map_err(|e| {
            flow_like_types::anyhow!("Failed to load .tflite model '{}': {}", model_path.path, e)
        })?;
        if raw.is_empty() {
            return Err(flow_like_types::anyhow!(
                "Model file '{}' is empty",
                model_path.path
            ));
        }

        let model_bytes: Vec<u8> = find_tflite_slice(&raw)
            .ok_or_else(|| flow_like_types::anyhow!("Could not locate TFLite buffer (TFL3 id)"))?
            .to_vec();

        let img = node_img.get_image(context).await?;
        let img_guard = img.lock().await;
        let rgb = img_guard.to_rgb8();
        let (src_w, src_h) = rgb.dimensions();
        let rgb_bytes = rgb.into_raw();
        drop(img_guard);

        let (top_idx, top_score) =
            tokio::task::spawn_blocking(move || -> flow_like_types::Result<(usize, f32)> {
                use tract_data::prelude::DatumType;

                // Load model
                let mut cursor = Cursor::new(model_bytes);
                let mut model = tract_tflite::tflite()
                    .model_for_read(&mut cursor)
                    .map_err(|e| flow_like_types::anyhow!("TFLite parse error: {e}"))?;

                // Prepare image (resize exactly with bicubic, like OpenCV INTER_CUBIC)
                let src = RgbImage::from_raw(src_w, src_h, rgb_bytes)
                    .ok_or_else(|| flow_like_types::anyhow!("Invalid source image buffer"))?;
                let resized = imageops::resize(&src, iw, ih, FilterType::CatmullRom);

                // Inspect input dtype and set input fact/shape
                let inlet = model.input_outlets()?[0];
                let orig = model.outlet_fact(inlet)?;
                let input_shape = tvec!(1, ih as usize, iw as usize, 3);
                let (fact, tensor): (TypedFact, Tensor) = match orig.datum_type {
                    DatumType::F32 => {
                        let arr = tract_ndarray::Array4::<f32>::from_shape_fn(
                            (1, ih as usize, iw as usize, 3),
                            |(_, y, x, c)| {
                                let p = resized.get_pixel(x as u32, y as u32);
                                p[c] as f32 / 127.5 - 1.0
                            },
                        );
                        (TypedFact::dt_shape(f32::datum_type(), input_shape), arr.into())
                    }
                    DatumType::U8 => {
                        let arr = tract_ndarray::Array4::<u8>::from_shape_fn(
                            (1, ih as usize, iw as usize, 3),
                            |(_, y, x, c)| {
                                let p = resized.get_pixel(x as u32, y as u32);
                                p[c]
                            },
                        );
                        (TypedFact::dt_shape(u8::datum_type(), input_shape), arr.into())
                    }
                    dt => {
                        return Err(flow_like_types::anyhow!(
                            "Unsupported input dtype: {dt:?} (only F32 and U8 are supported)"
                        ))
                    }
                };

                let mut runnable = model
                    .with_input_fact(0, fact)?
                    .into_optimized()?
                    .into_runnable()?;

                let outputs = runnable
                    .run(tvec!(tensor.into()))
                    .map_err(|e| flow_like_types::anyhow!("Failed to run TFLite model: {e}"))?;

                if outputs.is_empty() {
                    return Err(flow_like_types::anyhow!("Model produced no outputs"));
                }

                let out = outputs[0]
                    .to_array_view::<f32>()
                    .map_err(|e| flow_like_types::anyhow!("Output is not f32: {}", e))?;

                let mut best_idx = 0usize;
                let mut best_score = f32::MIN;
                for (i, v) in out.iter().enumerate() {
                    if *v > best_score {
                        best_idx = i;
                        best_score = *v;
                    }
                }
                Ok((best_idx, best_score))
            })
            .await
            .map_err(|e| flow_like_types::anyhow!("TFLite inference task join error: {}", e))??;

        let label = labels_vec.get(top_idx).cloned();
        let predictions = vec![ClassPrediction {
            class_idx: top_idx as u32,
            score: top_score,
            label,
        }];

        context
            .set_pin_value("predictions", json!(predictions))
            .await?;
        context.activate_exec_pin("exec_out").await?;
        Ok(())
    }
}

fn find_tflite_slice(buf: &[u8]) -> Option<&[u8]> {
    if buf.len() < 8 {
        return None;
    }
    let limit = buf.len() - 8;
    for i in 0..=limit {
        if &buf[i + 4..i + 8] == b"TFL3" {
            return Some(&buf[i..]);
        }
    }
    None
}

#[derive(Default)]
pub struct PredictionClassOrLabelNode {}

impl PredictionClassOrLabelNode {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl NodeLogic for PredictionClassOrLabelNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "ai_ml_pred_class_or_label",
            "Prediction Class/Label",
            "Extract class_idx and label from predictions.",
            "AI/ML",
        );
        node.add_icon("/flow/icons/find_model.svg");

        node.add_input_pin(
            "prediction",
            "Prediction",
            "Single ClassPrediction",
            VariableType::Struct,
        )
        .set_schema::<ClassPrediction>();

        node.add_output_pin(
            "class_idx",
            "Class Index",
            "Selected prediction class index",
            VariableType::Integer,
        );

        node.add_output_pin(
            "label",
            "Label",
            "Selected prediction label (empty if not provided)",
            VariableType::String,
        );

        node
    }

    async fn run(&self, context: &mut ExecutionContext) -> Result<()> {
        let prediction: ClassPrediction = context.evaluate_pin("prediction").await?;

        context.set_pin_value("class_idx", json!(prediction.class_idx)).await?;
        context
            .set_pin_value("label", json!(prediction.label.clone().unwrap_or_default()))
            .await?;
        Ok(())
    }
}

#[derive(Default)]
pub struct PredictionScoreNode {}

impl PredictionScoreNode {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl NodeLogic for PredictionScoreNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "ai_ml_pred_score",
            "Prediction Score",
            "Extract score from predictions.",
            "AI/ML/Teachable Machine",
        );
        node.add_icon("/flow/icons/find_model.svg");

        node.add_input_pin(
            "prediction",
            "Prediction",
            "Single ClassPrediction",
            VariableType::Struct,
        )
        .set_schema::<ClassPrediction>();

        node.add_output_pin(
            "score",
            "Score",
            "Selected prediction score",
            VariableType::Integer,
        );

        node
    }

    async fn run(&self, context: &mut ExecutionContext) -> Result<()> {
        let prediction: ClassPrediction = context.evaluate_pin("prediction").await?;

        context.set_pin_value("score", json!(prediction.score)).await?;
        Ok(())
    }
}

/// Register TFLite-related nodes
pub async fn register_functions() -> Vec<Arc<dyn NodeLogic>> {
    vec![
        Arc::new(TeachableMachineNode::default()) as Arc<dyn NodeLogic>,
        Arc::new(PredictionClassOrLabelNode::default()) as Arc<dyn NodeLogic>,
        Arc::new(PredictionScoreNode::default()) as Arc<dyn NodeLogic>,
    ]
}