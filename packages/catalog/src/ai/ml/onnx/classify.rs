use crate::{ai::ml::onnx::NodeOnnxSession, image::NodeImage};
use flow_like::{
    flow::{
        execution::{LogLevel, context::ExecutionContext},
        node::{Node, NodeLogic},
        pin::{PinOptions, ValueType},
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{
    Error, JsonSchema, Result, anyhow, async_trait,
    image::{DynamicImage, GenericImageView, imageops::FilterType},
    json::{Deserialize, Serialize, json},
};

use flow_like_model_provider::ml::{
    ndarray::{Array1, Array3, Array4, ArrayView1, Axis, s},
    ort::inputs,
};
use std::time::Instant;

/// # Classification Prediction
#[derive(Default, Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct Prediction {
    pub class_idx: u32,
    pub score: f32,
}

/// # DynamicImage to ONNX Input Tensor
/// Transforms:
///     1. Resize image to Input Size / Crop Percentage
///     2. CenterCrop image to Input Size
///     3. Scale pixel values to 0..1 floats
///     4. Normalize with mean and std deviation of the training dataset
///     5. As 4dim array (batch size x channels x width x height)
/// Reproduces https://github.com/huggingface/pytorch-image-models/blob/main/onnx_validate.py
fn img_to_arr(
    img: &DynamicImage,
    arr_width: u32,
    arr_height: u32,
    crop_pct: f32,
    mean: &[f32; 3],
    std: &[f32; 3],
) -> Result<Array4<f32>, Error> {
    let (img_width, img_height) = img.dimensions();

    // first resize, then crop a centered square from resized such that cropped/resized = crop_pct and cropped = ONNX input shape
    let buf_u8 = if (img_width == arr_width) && (img_height == arr_height) && crop_pct > 0.999 {
        // allow users to do resizing and cropping outside this node
        img.to_rgb8().into_raw()
    } else {
        let arr_width_f = arr_width as f32;
        let arr_height_f = arr_height as f32;

        // determine resize dims such that when we crop in the following step we get an arr_width x arr_height cutout
        let resize_width = arr_width_f / crop_pct;
        let resize_height = arr_height_f / crop_pct;
        println!("{:?}, {:?}", resize_width, resize_height);

        // match smaller edge of image to target resize dimension
        let resize_width = if img_width > img_height {
            resize_width * (img_width as f32 / img_height as f32)
        } else {
            resize_width
        };

        let resize_height = if img_height > img_width {
            resize_height * (img_height as f32 / img_width as f32)
        } else {
            resize_height
        };

        // top-left corner of center crop box
        let x = (resize_width - arr_width_f) / 2.0;
        let y = (resize_height - arr_height_f) / 2.0;

        let img_cropped = img
            .resize(
                resize_width as u32,
                resize_height as u32,
                FilterType::CatmullRom,
            ) // pytorch default bicubic
            .crop_imm(x as u32, y as u32, arr_width, arr_height);

        img_cropped.into_rgb8().into_raw()
    };

    // to float tensor
    let buf_f32: Vec<f32> = buf_u8.iter().map(|&v| (v as f32) / 255.0).collect();
    let arr3 = Array3::from_shape_vec((arr_height as usize, arr_width as usize, 3), buf_f32)?;

    // normalize per channel
    let mut arr3 = arr3; // make mutable
    for c in 0..3 {
        arr3.slice_mut(s![.., .., c]).map_inplace(|x| {
            *x = (*x - mean[c]) / std[c];
        });
    }

    // expand into 4dim array
    let arr4 = arr3.permuted_axes([2, 0, 1]).insert_axis(Axis(0));
    Ok(arr4)
}

/// # Apply Softmax on ONNX output logits
/// -> all class channels scaled between 0..1 and sum over all classes = 1
fn softmax(input_array: ArrayView1<f32>) -> Result<Array1<f32>, Error> {
    let max_value = input_array.fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let exp_shifted = input_array.mapv(|x| (x - max_value).exp());
    let sum_exp = exp_shifted.sum();
    Ok(exp_shifted / sum_exp)
}

/// # Image Classification
/// Predict classes for images
/// Tested for ONNX models exported via https://github.com/huggingface/pytorch-image-models
/// Applies default input image transformations for close reproduction of PyTorch results
#[derive(Default)]
pub struct ImageClassificationNode {}

impl ImageClassificationNode {
    pub fn new() -> Self {
        ImageClassificationNode {}
    }
}

#[async_trait]
impl NodeLogic for ImageClassificationNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "image_classification",
            "Image Classification",
            "Image Classification with ONNX-Models",
            "AI/ML/ONNX",
        );

        node.add_icon("/flow/icons/find_model.svg");

        // inputs
        node.add_input_pin(
            "exec_in",
            "Input",
            "Initiate Execution",
            VariableType::Execution,
        );

        node.add_input_pin("model", "Model", "ONNX Model Session", VariableType::Struct)
            .set_schema::<NodeOnnxSession>()
            .set_options(PinOptions::new().set_enforce_schema(true).build());

        node.add_input_pin("image_in", "Image", "Image Object", VariableType::Struct)
            .set_schema::<NodeImage>()
            .set_options(PinOptions::new().set_enforce_schema(true).build());

        node.add_input_pin(
            "mean",
            "Mean",
            "Image Mean for Normalization (per channel)",
            VariableType::Float,
        )
        .set_value_type(ValueType::Array)
        .set_default_value(Some(json!(vec![0.4850, 0.4560, 0.4060]))); // ImageNet defaults

        node.add_input_pin(
            "std",
            "Std",
            "Image Standard Deviation for Normalization (per channel)",
            VariableType::Float,
        )
        .set_value_type(ValueType::Array)
        .set_default_value(Some(json!(vec![0.2290, 0.2240, 0.2250]))); // ImageNet defaults

        node.add_input_pin(
            "crop_pct",
            "Crop",
            "Center Crop Percentage",
            VariableType::Float,
        )
        .set_options(PinOptions::new().set_range((0., 1.)).build())
        .set_default_value(Some(json!(0.875)));

        node.add_input_pin(
            "softmax",
            "Softmax?",
            "Scale Outputs with Softmax",
            VariableType::Boolean,
        )
        .set_default_value(Some(json!(true)));

        // outputs
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
        .set_value_type(flow_like::flow::pin::ValueType::Array);

        node
    }

    async fn run(&self, context: &mut ExecutionContext) -> Result<()> {
        context.deactivate_exec_pin("exec_out").await?;

        // fetch inputs
        let node_session: NodeOnnxSession = context.evaluate_pin("model").await?;
        let node_img: NodeImage = context.evaluate_pin("image_in").await?;
        let mean_vec: Vec<f32> = context.evaluate_pin("mean").await?;
        let std_vec: Vec<f32> = context.evaluate_pin("std").await?;
        let crop_pct: f32 = context.evaluate_pin("crop_pct").await?;
        let apply_softmax: bool = context.evaluate_pin("softmax").await?;

        let mean = <&[f32; 3]>::try_from(mean_vec.as_slice())?;
        let std = <&[f32; 3]>::try_from(std_vec.as_slice())?;

        let (outputs, t0) = {
            // acquire onnx session
            let session = node_session.get_session(context).await?;
            let session_guard = session.lock().await;

            // transform DynamicImage -> Array
            let t0 = Instant::now();
            let arr_in = {
                let img = node_img.get_image(context).await?;
                let img_guard = img.lock().await;

                img_to_arr(
                    &img_guard,
                    session_guard.input_width,
                    session_guard.input_height,
                    crop_pct,
                    mean,
                    std,
                )?
            }; // drop img_guard

            let inputs = match inputs![&session_guard.input_name => arr_in.view()] {
                Ok(mapping) => Ok(mapping),
                Err(_) => Err(anyhow!(
                    "Failed to put input image into ONNX model input tensor"
                )),
            }?;
            let dt = t0.elapsed();
            context.log_message(&format!("[preprocessing]: {:?}", dt), LogLevel::Debug);

            // inference
            let t0 = Instant::now();
            let outputs = session_guard.session.run(inputs)?;
            let arr_out =
                outputs[session_guard.output_name.as_str()].try_extract_tensor::<f32>()?;
            let dt = t0.elapsed();
            context.log_message(&format!("[inference]: {:?}", dt), LogLevel::Debug);

            // postprocessing
            let t0 = Instant::now();
            let outputs = arr_out.reversed_axes();
            let outputs = outputs.slice(s![.., 0]);

            let outputs = if apply_softmax {
                softmax(outputs)?
            } else {
                outputs.to_owned()
            };

            (outputs, t0)
        }; // drop session guard

        let mut predictions = Vec::with_capacity(outputs.len_of(Axis(0)));
        for (class_idx, score) in outputs.axis_iter(Axis(0)).enumerate() {
            let score = score.first().copied().unwrap_or(0.);
            predictions.push(Prediction {
                class_idx: class_idx as u32,
                score,
            });
        }
        predictions.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let dt = t0.elapsed();
        context.log_message(&format!("[postprocessing]: {:?}", dt), LogLevel::Debug);

        // set outputs
        context
            .set_pin_value("predictions", json!(predictions))
            .await?;
        context.activate_exec_pin("exec_out").await?;
        Ok(())
    }
}
