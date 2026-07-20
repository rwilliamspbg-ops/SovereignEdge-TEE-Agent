//! Local inference backends for the edge agent
//!
//! The agent's local inference path is pluggable via [`InferenceBackend`]:
//! - [`SimulatedBackend`] returns canned results (default, no dependencies)
//! - `LlamaCppBackend` runs a real GGUF model via llama.cpp
//!   (enable with `--features llama`; add GPU offload by building
//!   llama-cpp-2 with its `cuda`/`vulkan` features)

use crate::AgentError;

/// Output of a local inference call
#[derive(Debug, Clone)]
pub struct InferenceOutput {
    pub text: String,
    pub tokens_generated: usize,
    pub model_name: String,
}

/// Pluggable local inference backend
pub trait InferenceBackend: Send {
    /// Short identifier for logs and result metadata
    fn name(&self) -> &str;

    /// Run inference on a prompt, generating at most `max_tokens` tokens
    fn infer(&mut self, prompt: &str, max_tokens: usize) -> Result<InferenceOutput, AgentError>;
}

/// Default backend: canned safety-check response, no model required
pub struct SimulatedBackend;

impl InferenceBackend for SimulatedBackend {
    fn name(&self) -> &str {
        "simulated"
    }

    fn infer(&mut self, _prompt: &str, _max_tokens: usize) -> Result<InferenceOutput, AgentError> {
        Ok(InferenceOutput {
            text: "LOCAL_SAFETY_CHECK".to_string(),
            tokens_generated: 0,
            model_name: "qwen2.5-0.5b (simulated)".to_string(),
        })
    }
}

#[cfg(feature = "llama")]
pub use llama::LlamaCppBackend;

#[cfg(feature = "llama")]
mod llama {
    use super::{InferenceBackend, InferenceOutput};
    use crate::AgentError;
    use llama_cpp_2::context::params::LlamaContextParams;
    use llama_cpp_2::llama_backend::LlamaBackend;
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::params::LlamaModelParams;
    use llama_cpp_2::model::{AddBos, LlamaModel};
    use llama_cpp_2::sampling::LlamaSampler;
    use std::num::NonZeroU32;
    use std::path::Path;
    use tracing::info;

    const CTX_SIZE: u32 = 512;

    /// Real local inference via llama.cpp (GGUF models)
    pub struct LlamaCppBackend {
        backend: LlamaBackend,
        model: LlamaModel,
        model_name: String,
    }

    impl LlamaCppBackend {
        /// Load a GGUF model. `gpu_layers` > 0 offloads that many
        /// transformer layers to the GPU (no-op in CPU-only builds).
        pub fn load(model_path: &Path, gpu_layers: u32) -> Result<Self, AgentError> {
            let backend = LlamaBackend::init()
                .map_err(|e| AgentError::InferenceFailed(format!("llama init: {}", e)))?;

            let model_params = LlamaModelParams::default().with_n_gpu_layers(gpu_layers);
            let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
                .map_err(|e| AgentError::InferenceFailed(format!("model load: {}", e)))?;

            let model_name = model_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            info!(
                "[LlamaCpp] Loaded model '{}' ({} GPU layers requested)",
                model_name, gpu_layers
            );

            Ok(Self {
                backend,
                model,
                model_name,
            })
        }
    }

    impl InferenceBackend for LlamaCppBackend {
        fn name(&self) -> &str {
            "llama.cpp"
        }

        fn infer(
            &mut self,
            prompt: &str,
            max_tokens: usize,
        ) -> Result<InferenceOutput, AgentError> {
            let ctx_params = LlamaContextParams::default().with_n_ctx(NonZeroU32::new(CTX_SIZE));
            let mut ctx = self
                .model
                .new_context(&self.backend, ctx_params)
                .map_err(|e| AgentError::InferenceFailed(format!("context: {}", e)))?;

            let tokens = self
                .model
                .str_to_token(prompt, AddBos::Always)
                .map_err(|e| AgentError::InferenceFailed(format!("tokenize: {}", e)))?;

            let mut batch = LlamaBatch::new(CTX_SIZE as usize, 1);
            let last_idx = tokens.len().saturating_sub(1);
            for (i, token) in tokens.iter().enumerate() {
                batch
                    .add(*token, i as i32, &[0], i == last_idx)
                    .map_err(|e| AgentError::InferenceFailed(format!("batch add: {}", e)))?;
            }
            ctx.decode(&mut batch)
                .map_err(|e| AgentError::InferenceFailed(format!("decode: {}", e)))?;

            let mut sampler = LlamaSampler::greedy();
            let mut utf8_decoder = encoding_rs::UTF_8.new_decoder();
            let mut output = String::new();
            let mut n_cur = tokens.len() as i32;
            let mut generated = 0usize;

            while generated < max_tokens && (n_cur as u32) < CTX_SIZE {
                let token = sampler.sample(&ctx, batch.n_tokens() - 1);
                if self.model.is_eog_token(token) {
                    break;
                }

                let piece = self
                    .model
                    .token_to_piece(token, &mut utf8_decoder, false, None)
                    .unwrap_or_default();
                output.push_str(&piece);
                generated += 1;

                batch.clear();
                batch
                    .add(token, n_cur, &[0], true)
                    .map_err(|e| AgentError::InferenceFailed(format!("batch add: {}", e)))?;
                ctx.decode(&mut batch)
                    .map_err(|e| AgentError::InferenceFailed(format!("decode: {}", e)))?;
                n_cur += 1;
            }

            Ok(InferenceOutput {
                text: output,
                tokens_generated: generated,
                model_name: self.model_name.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulated_backend() {
        let mut backend = SimulatedBackend;
        let result = backend.infer("test prompt", 16).unwrap();
        assert_eq!(result.text, "LOCAL_SAFETY_CHECK");
        assert_eq!(backend.name(), "simulated");
    }
}
