use crate::configs::stt::AvailableSTTModel;
use crate::configs::vad::AvailableVADModel;

/// Max and min duration for pause threshold in chunks
/// We don't provide specific numbers to the user as it is harder to map chunks to seconds
/// and would make it harder.
const SHORT_PAUSE_DURATION: u32 = 16;
const LONG_PAUSE_DURATION: u32 = 32;

/// Which field is currently focused in the settings screen
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SettingsField {
    #[default]
    InputDevice,
    VadThreshold,
    MinFragmentumDurationSeconds,
    MaxFragmentumDurationSeconds,
    PauseThresholdInChunks,
    STTModel,
    VADModel,
}

/// State for the settings screen
#[derive(Debug, Clone)]
pub struct SettingsState {
    /// List of available input device names
    pub available_devices: Vec<String>,
    /// Currently selected device index (0 = system default, 1+ = specific devices)
    pub selected_device_index: usize,
    /// Current VAD threshold value (0.0 to 1.0)
    pub vad_threshold: f32,
    /// Current minimum length in seconds for fragmenta
    pub min_fragmentum_duration_seconds: f32,
    /// Current maximum length in seconds for fragmenta
    pub max_fragmentum_duration_seconds: f32,
    /// Current length in chunks to trigger a pause
    pub pause_threshold_in_chunks: u32,
    /// Which field is currently focused
    pub active_field: SettingsField,
    /// List of available STT model keys (from models.toml)
    pub available_stt_models: Vec<String>,
    /// Currently selected STT model index
    pub selected_stt_model_index: usize,
    /// List of available VAD model keys (from models.toml)
    pub available_vad_models: Vec<String>,
    /// Currently selected VAD model index
    pub selected_vad_model_index: usize,
}

impl SettingsState {
    /// Create a new SettingsState by enumerating devices and loading current config values
    pub fn new(
        available_devices: Vec<String>,
        current_device: Option<&str>,
        vad_threshold: f32,
        min_fragmentum_duration_seconds: f32,
        max_fragmentum_duration_seconds: f32,
        pause_threshold_in_chunks: u32,
        available_stt_models: Vec<String>,
        current_stt_model: &AvailableSTTModel,
        available_vad_models: Vec<String>,
        current_vad_model: &AvailableVADModel,
    ) -> Self {
        // Find the index of the current device, defaulting to 0 (system default)
        let selected_device_index = if let Some(device_name) = current_device {
            // Index 0 is "System Default", so actual devices start at index 1
            available_devices
                .iter()
                .position(|d| d == device_name)
                .map(|idx| idx + 1)
                .unwrap_or(0)
        } else {
            0
        };

        // Find the index of the current STT model
        let current_stt_key = current_stt_model.as_key();
        let selected_stt_model_index = available_stt_models
            .iter()
            .position(|m| m == current_stt_key)
            .unwrap_or(0);

        // Find the index of the current VAD model
        let current_vad_key = current_vad_model.as_key();
        let selected_vad_model_index = available_vad_models
            .iter()
            .position(|m| m == current_vad_key)
            .unwrap_or(0);

        Self {
            available_devices,
            selected_device_index,
            vad_threshold,
            min_fragmentum_duration_seconds,
            max_fragmentum_duration_seconds,
            pause_threshold_in_chunks,
            active_field: SettingsField::default(),
            available_stt_models,
            selected_stt_model_index,
            available_vad_models,
            selected_vad_model_index,
        }
    }

    /// Get the currently selected device name (None means system default)
    pub fn selected_device_name(&self) -> Option<&str> {
        if self.selected_device_index == 0 {
            None
        } else {
            self.available_devices
                .get(self.selected_device_index - 1)
                .map(|s| s.as_str())
        }
    }

    /// Get display name for the currently selected device
    pub fn selected_device_display(&self) -> &str {
        if self.selected_device_index == 0 {
            "System Default"
        } else {
            self.available_devices
                .get(self.selected_device_index - 1)
                .map(|s| s.as_str())
                .unwrap_or("Unknown")
        }
    }

    /// Total number of device options (system default + all enumerated devices)
    pub fn device_count(&self) -> usize {
        self.available_devices.len() + 1
    }

    /// Select the next device
    pub fn next_device(&mut self) {
        self.selected_device_index = (self.selected_device_index + 1) % self.device_count();
    }

    /// Select the previous device
    pub fn previous_device(&mut self) {
        if self.selected_device_index == 0 {
            self.selected_device_index = self.device_count() - 1;
        } else {
            self.selected_device_index -= 1;
        }
    }

    /// Decrease VAD threshold by 0.05, capped at 0.0
    pub fn decrease_threshold(&mut self) {
        self.vad_threshold = (self.vad_threshold - 0.05).max(0.0);
    }

    /// Increase VAD threshold by 0.05, capped at 1.0
    pub fn increase_threshold(&mut self) {
        self.vad_threshold = (self.vad_threshold + 0.05).min(1.0);
    }

    /// Decrease minimum fragmentum duration in seconds. Can't be less than 1 second.
    pub fn decrease_min_fragmentum_duration(&mut self) {
        self.min_fragmentum_duration_seconds =
            (self.min_fragmentum_duration_seconds - 1.0).max(1.0);
    }

    /// Increase minimum fragmentum duration in seconds. Can't be more than the max duration
    pub fn increase_min_fragmentum_duration(&mut self) {
        self.min_fragmentum_duration_seconds =
            (self.min_fragmentum_duration_seconds + 1.0).min(self.max_fragmentum_duration_seconds);
    }

    /// Decrease minimum fragmentum duration in seconds. Can't be less than 1 second.
    pub fn decrease_max_fragmentum_duration(&mut self) {
        self.max_fragmentum_duration_seconds =
            (self.max_fragmentum_duration_seconds - 1.0).max(self.min_fragmentum_duration_seconds);
    }

    /// Increase maximum fragmentum duration in seconds. Can't be more than 60s
    pub fn increase_max_fragmentum_duration(&mut self) {
        self.max_fragmentum_duration_seconds =
            (self.max_fragmentum_duration_seconds + 1.0).min(60.0);
    }

    /// Decrease pause threshold measured in chunks
    pub fn decrease_pause_threshold(&mut self) {
        self.pause_threshold_in_chunks =
            (self.pause_threshold_in_chunks - 8_u32).max(SHORT_PAUSE_DURATION);
    }

    /// Increase pause threshold measured in chunks
    pub fn increase_pause_threshold(&mut self) {
        self.pause_threshold_in_chunks =
            (self.pause_threshold_in_chunks + 8_u32).min(LONG_PAUSE_DURATION);
    }

    /// Cycle to the next field
    pub fn next_field(&mut self) {
        self.active_field = match self.active_field {
            SettingsField::InputDevice => SettingsField::VadThreshold,
            SettingsField::VadThreshold => SettingsField::MinFragmentumDurationSeconds,
            SettingsField::MinFragmentumDurationSeconds => {
                SettingsField::MaxFragmentumDurationSeconds
            }
            SettingsField::MaxFragmentumDurationSeconds => SettingsField::PauseThresholdInChunks,
            SettingsField::PauseThresholdInChunks => SettingsField::STTModel,
            SettingsField::STTModel => SettingsField::VADModel,
            SettingsField::VADModel => SettingsField::InputDevice,
        };
    }

    /// Cycle to the previous field
    pub fn previous_field(&mut self) {
        self.active_field = match self.active_field {
            SettingsField::InputDevice => SettingsField::VADModel,
            SettingsField::VadThreshold => SettingsField::InputDevice,
            SettingsField::MinFragmentumDurationSeconds => SettingsField::VadThreshold,
            SettingsField::MaxFragmentumDurationSeconds => {
                SettingsField::MinFragmentumDurationSeconds
            }
            SettingsField::PauseThresholdInChunks => SettingsField::MaxFragmentumDurationSeconds,
            SettingsField::STTModel => SettingsField::PauseThresholdInChunks,
            SettingsField::VADModel => SettingsField::STTModel,
        };
    }

    /// Get the currently selected STT model key
    pub fn selected_stt_model_key(&self) -> Option<&str> {
        self.available_stt_models
            .get(self.selected_stt_model_index)
            .map(|s| s.as_str())
    }

    /// Get display name for the currently selected STT model
    pub fn selected_stt_model_display(&self) -> &str {
        self.available_stt_models
            .get(self.selected_stt_model_index)
            .map(|s| s.as_str())
            .unwrap_or("Unknown")
    }

    /// Select the next STT model
    pub fn next_stt_model(&mut self) {
        if !self.available_stt_models.is_empty() {
            self.selected_stt_model_index =
                (self.selected_stt_model_index + 1) % self.available_stt_models.len();
        }
    }

    /// Select the previous STT model
    pub fn previous_stt_model(&mut self) {
        if !self.available_stt_models.is_empty() {
            if self.selected_stt_model_index == 0 {
                self.selected_stt_model_index = self.available_stt_models.len() - 1;
            } else {
                self.selected_stt_model_index -= 1;
            }
        }
    }

    /// Get the currently selected VAD model key
    pub fn selected_vad_model_key(&self) -> Option<&str> {
        self.available_vad_models
            .get(self.selected_vad_model_index)
            .map(|s| s.as_str())
    }

    /// Get display name for the currently selected VAD model
    pub fn selected_vad_model_display(&self) -> &str {
        self.available_vad_models
            .get(self.selected_vad_model_index)
            .map(|s| s.as_str())
            .unwrap_or("Unknown")
    }

    /// Select the next VAD model
    pub fn next_vad_model(&mut self) {
        if !self.available_vad_models.is_empty() {
            self.selected_vad_model_index =
                (self.selected_vad_model_index + 1) % self.available_vad_models.len();
        }
    }

    /// Select the previous VAD model
    pub fn previous_vad_model(&mut self) {
        if !self.available_vad_models.is_empty() {
            if self.selected_vad_model_index == 0 {
                self.selected_vad_model_index = self.available_vad_models.len() - 1;
            } else {
                self.selected_vad_model_index -= 1;
            }
        }
    }
}
