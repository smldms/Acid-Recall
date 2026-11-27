//! Select graphics APIs before creating the native instance. A driver access violation
//! during Vulkan enumeration cannot be recovered by ordinary in-process fallback.

use eframe::egui_wgpu::{WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu::Backends;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Dx12,
    Vulkan,
    Auto,
}

impl Backend {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "dx12" => Some(Self::Dx12),
            "vulkan" => Some(Self::Vulkan),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

fn selected_backends(
    requested: Option<Backend>,
    environment: Option<Backends>,
    windows: bool,
) -> Option<Backends> {
    match requested {
        Some(Backend::Dx12) => Some(Backends::DX12),
        Some(Backend::Vulkan) => Some(Backends::VULKAN),
        Some(Backend::Auto) => Some(Backends::PRIMARY | Backends::GL),
        None => environment
            .filter(|backends| !backends.is_empty())
            .or_else(|| windows.then_some(Backends::DX12)),
    }
}

fn configuration_for(
    requested: Option<Backend>,
    environment: Option<Backends>,
    windows: bool,
) -> eframe::WgpuConfiguration {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    if let Some(backends) = selected_backends(requested, environment, windows) {
        setup.instance_descriptor.backends = backends;
    }
    eframe::WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

pub fn configuration(requested: Option<Backend>) -> eframe::WgpuConfiguration {
    configuration_for(requested, Backends::from_env(), cfg!(windows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_instance_disables_vulkan_before_any_adapter_is_enumerated() {
        for environment in [None, Some(Backends::empty())] {
            let config = configuration_for(None, environment, true);
            let WgpuSetup::CreateNew(setup) = config.wgpu_setup else {
                panic!("a fresh instance is required");
            };
            assert_eq!(setup.instance_descriptor.backends, Backends::DX12);
            assert!(
                !setup
                    .instance_descriptor
                    .backends
                    .contains(Backends::VULKAN)
            );
        }
    }

    #[test]
    fn explicit_command_line_overrides_environment_and_auto_is_opt_in() {
        assert_eq!(
            selected_backends(Some(Backend::Dx12), Some(Backends::VULKAN), true),
            Some(Backends::DX12)
        );
        assert_eq!(
            selected_backends(Some(Backend::Vulkan), Some(Backends::DX12), true),
            Some(Backends::VULKAN)
        );
        assert_eq!(
            selected_backends(Some(Backend::Auto), Some(Backends::DX12), true),
            Some(Backends::PRIMARY | Backends::GL)
        );
        assert_eq!(
            selected_backends(None, Some(Backends::VULKAN), true),
            Some(Backends::VULKAN)
        );
    }

    #[test]
    fn other_platforms_keep_their_original_default_and_environment_choices() {
        assert_eq!(selected_backends(None, None, false), None);
        assert_eq!(
            selected_backends(None, Some(Backends::METAL), false),
            Some(Backends::METAL)
        );
    }
}
