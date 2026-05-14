//! `webrtc::api::API` setup. Built once and shared by every `PeerConnection`.

use std::sync::Arc;

use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::{APIBuilder, API};
use webrtc::interceptor::registry::Registry;

use crate::SfuError;

pub fn build(setting_engine: Option<SettingEngine>) -> Result<Arc<API>, SfuError> {
    let mut media = MediaEngine::default();
    media
        .register_default_codecs()
        .map_err(|e| SfuError::Engine(e.to_string()))?;

    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut media)
        .map_err(|e| SfuError::Engine(e.to_string()))?;

    let mut builder = APIBuilder::new()
        .with_media_engine(media)
        .with_interceptor_registry(registry);
    if let Some(se) = setting_engine {
        builder = builder.with_setting_engine(se);
    }
    Ok(Arc::new(builder.build()))
}
