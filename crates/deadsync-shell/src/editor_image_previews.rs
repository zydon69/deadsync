//! Application-owned editor image service. Keep one instance across editor visits.
use deadlib_assets::AssetManager;
use deadlib_render::Backend;
use deadsync_theme_simply_love::screens::edition::image_previews::{Cache, Role, Status};
use std::{sync::Arc, time::Duration};

pub(crate) use deadsync_theme_simply_love::screens::edition::image_previews::{
    PreviewRequest as Request, PreviewView as View,
};

pub(crate) struct Service {
    cache: Cache,
    document_identity: Option<u64>,
    retry_after: Duration,
    last_upload_failure: Option<String>,
}

impl Service {
    pub(crate) fn new() -> std::io::Result<Self> {
        Ok(Self {
            cache: Cache::new()?,
            document_identity: None,
            retry_after: Duration::ZERO,
            last_upload_failure: None,
        })
    }

    /// Call on every editor update, including None when the editor closes.
    /// Paths are absolute or already resolved against the current simfile folder.
    pub(crate) fn set_request(&mut self, request: Option<Request>) {
        let identity = request.as_ref().map(|request| request.document_identity);
        if self.document_identity != identity {
            self.cache.set_paths([None, None]);
            self.document_identity = identity;
        }
        self.cache
            .set_paths(request.map_or([None, None], |request| request.paths));
    }

    /// Run on the renderer owner thread before collecting the screen's actors.
    /// Retirements cancel queued uploads before replacement frames are submitted.
    pub(crate) fn update(
        &mut self,
        time: Duration,
        assets: &mut AssetManager,
        backend: &mut Backend,
    ) -> Result<(), String> {
        self.cache.update(time);
        for key in self.cache.take_retired_keys() {
            if let Some((_, texture)) = assets.unload_texture_keep_handle(&key) {
                backend.retire_texture(texture);
            }
        }
        // A renderer rebuild creates a fresh asset store. Reupload retained CPU
        // frames; no second decoder or new generated-texture identity is needed.
        if Role::ALL.iter().any(|role| {
            let view = self.cache.view(*role);
            view.has_resident_image && !assets.has_uploaded_texture_key(view.key)
        }) {
            self.cache.refresh_uploads();
        }
        if time < self.retry_after {
            return Ok(());
        }
        for upload in self.cache.take_uploads() {
            if let Err(error) = assets.update_texture_for_key(backend, &upload.key, &upload.image) {
                self.cache.refresh_uploads();
                self.retry_after = time + Duration::from_secs(2);
                let message = error.to_string();
                if self.last_upload_failure.as_ref() != Some(&message) {
                    self.last_upload_failure = Some(message.clone());
                    return Err(message);
                }
                return Ok(());
            }
        }
        self.last_upload_failure = None;
        Ok(())
    }

    pub(crate) fn views(&self) -> [View; 2] {
        Role::ALL.map(|role| {
            let view = self.cache.view(role);
            View {
                status: view.status.clone(),
                key: Arc::from(view.key),
                source_size: view.source_size,
                has_resident_image: view.has_resident_image,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn document_switch_and_close_invalidate_paths_without_replacing_service() {
        let mut service = Service::new().unwrap();
        let keys = service.cache.texture_keys();
        let request = Request {
            document_identity: 1,
            paths: [Some(PathBuf::from("missing.png")), None],
        };
        service.set_request(Some(request.clone()));
        assert_eq!(service.views()[0].status, Status::Loading);
        service.set_request(None);
        assert_eq!(service.views()[0].status, Status::Empty);
        for identity in 2..258 {
            service.set_request(Some(Request {
                document_identity: identity,
                ..request.clone()
            }));
            assert_eq!(service.cache.texture_keys(), keys);
            service.set_request(None);
            assert!(!service.views()[0].has_resident_image);
        }
    }
}
