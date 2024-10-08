use std::cell::{Ref, RefCell};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use crate::{done, fail, info, warn, NiceUnwrap};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Profile {
	pub name: String,
	pub gd_path: PathBuf,

	#[serde(default = "profile_platform_default")]
	pub platform: String,

	#[serde(flatten)]
	other: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
	pub current_profile: Option<String>,
	pub profiles: Vec<RefCell<Profile>>,
	pub default_developer: Option<String>,
	pub sdk_nightly: bool,
	pub sdk_version: Option<String>,
	pub index_token: Option<String>,
	#[serde(default = "default_index_url")]
	pub index_url: String,
	#[serde(flatten)]
	other: HashMap<String, Value>,
}

fn default_index_url() -> String {
	"https://api.sapfire-sdk.org".to_string()
}

// old config.json structures for migration
// TODO: remove this in 3.0
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct OldConfigInstallation {
	pub path: PathBuf,
	pub executable: String,
}

// TODO: remove this in 3.0
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct OldConfig {
	pub default_installation: usize,
	pub working_installation: Option<usize>,
	pub installations: Option<Vec<OldConfigInstallation>>,
	pub default_developer: Option<String>,
}

fn profile_platform_default() -> String {
	if cfg!(target_os = "windows") {
		"win".to_owned()
	} else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
		"mac-intel".to_owned()
	} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
		"mac-arm".to_owned()
	} else {
		"win".to_owned()
	}
}

// TODO: remove this in 3.0
impl OldConfig {
	pub fn migrate(&self) -> Config {
		let profiles = self
			.installations
			.as_ref()
			.map(|insts| {
				insts
					.iter()
					.map(|inst| {
						RefCell::from(Profile {
							name: inst
								.executable
								.strip_suffix(".exe")
								.unwrap_or(&inst.executable)
								.into(),
							gd_path: inst.path.clone(),
							platform: String::from("win"),
							other: HashMap::new(),
						})
					})
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		Config {
			current_profile: profiles
				.get(
					self.working_installation
						.unwrap_or(self.default_installation),
				)
				.map(|i| i.borrow().name.clone()),
			profiles,
			default_developer: self.default_developer.to_owned(),
			sdk_nightly: false,
			sdk_version: None,
			other: HashMap::new(),
			index_token: None,
			index_url: "https://api.sapfire-sdk.org".to_string(),
		}
	}
}

pub fn sapfire_root() -> PathBuf {
	// get data dir per-platform
	let data_dir: PathBuf;
	#[cfg(any(windows, target_os = "linux", target_os = "android"))]
	{
		data_dir = dirs::data_local_dir().unwrap().join("Sapfire");
	};
	#[cfg(target_os = "macos")]
	{
		data_dir = PathBuf::from("/Users/Shared/Sapfire");
	};
	#[cfg(not(any(
		windows,
		target_os = "macos",
		target_os = "linux",
		target_os = "android"
	)))]
	{
		use std::compile_error;
		compile_error!("implement root directory");
	};
	data_dir
}

fn migrate_location(name: &str, mut path: PathBuf, platform: &str) -> PathBuf {
	// Migrate folder to executable
	if (platform == "win") && path.is_dir() {
		path.push("GeometryDash.exe");

		if !path.exists() {
			warn!(
				"Unable to find GeometryDash.exe in profile \
				  '{}', please update the GD path for it",
				name
			);
		}
	} else if path.file_name().unwrap() == "Contents" {
		path = path.parent().unwrap().to_path_buf();
	}

	path
}

impl Profile {
	pub fn new(name: String, location: PathBuf, platform: String) -> Profile {
		Profile {
			gd_path: migrate_location(&name, location, &platform),
			name,
			platform,
			other: HashMap::<String, Value>::new(),
		}
	}

	pub fn gd_dir(&self) -> PathBuf {
		if self.platform == "win" {
			self.gd_path.parent().unwrap().to_path_buf()
		} else {
			self.gd_path.clone()
		}
	}

	pub fn sapfire_dir(&self) -> PathBuf {
		if self.platform == "win" {
			self.gd_path.parent().unwrap().join("sapfire")
		} else if self.platform == "android32" || self.platform == "android64" {
			self.gd_path.join("game/sapfire")
		} else {
			self.gd_path.join("Contents/sapfire")
		}
	}

	pub fn mods_dir(&self) -> PathBuf {
		self.sapfire_dir().join("mods")
	}

	pub fn platform_str(&self) -> &str {
		self.platform.as_str()
	}
}

impl Config {
	pub fn get_profile(&self, name: &Option<String>) -> Option<&RefCell<Profile>> {
		if let Some(name) = name {
			self.profiles.iter().find(|x| &x.borrow().name == name)
		} else {
			None
		}
	}

	pub fn get_current_profile(&self) -> Ref<Profile> {
		self.get_profile(&self.current_profile)
			.nice_unwrap("No current profile found!")
			.borrow()
	}

	pub fn try_sdk_path() -> Result<PathBuf, String> {
		let sdk_var = std::env::var("SAPFIRE_SDK").map_err(|_| {
			"Unable to find Sapfire SDK (SAPFIRE_SDK isn't set). Please install \
				it using `sapfire sdk install` or use `sapfire sdk set-path` to set \
				it to an existing clone. If you just installed the SDK using \
				`sapfire sdk install`, please restart your terminal / computer to \
				apply changes."
		})?;

		let path = PathBuf::from(sdk_var);
		if !path.is_dir() {
			return Err(format!(
				"Internal Error: SAPFIRE_SDK doesn't point to a directory ({}). This \
				might be caused by having run `sapfire sdk set-path` - try restarting \
				your terminal / computer, or reinstall using `sapfire sdk install --reinstall`",
				path.display()
			));
		}
		if !path.join("VERSION").exists() {
			return Err(
				"Internal Error: SAPFIRE_SDK/VERSION not found. Please reinstall \
				the Sapfire SDK using `sapfire sdk install --reinstall`"
					.into(),
			);
		}

		Ok(path)
	}

	pub fn sdk_path() -> PathBuf {
		Self::try_sdk_path().nice_unwrap("Unable to get SDK path")
	}

	/// Path to cross-compilation tools
	pub fn cross_tools_path() -> PathBuf {
		sapfire_root().join("cross-tools")
	}

	pub fn new() -> Config {
		if !sapfire_root().exists() {
			warn!("It seems you don't have Sapfire installed. Some operations will not work");
			warn!("You can setup Sapfire using `sapfire config setup`");

			return Config {
				current_profile: None,
				profiles: Vec::new(),
				default_developer: None,
				sdk_nightly: false,
				sdk_version: None,
				other: HashMap::<String, Value>::new(),
				index_token: None,
				index_url: "https://api.sapfire-sdk.org".to_string(),
			};
		}

		let config_json = sapfire_root().join("config.json");

		let mut output: Config = if !config_json.exists() {
			info!("Setup Sapfire using `sapfire config setup`");
			// Create new config
			Config {
				current_profile: None,
				profiles: Vec::new(),
				default_developer: None,
				sdk_nightly: false,
				sdk_version: None,
				index_token: None,
				other: HashMap::<String, Value>::new(),
				index_url: "https://api.sapfire-sdk.org".to_string(),
			}
		} else {
			// Parse config
			let config_json_str =
				&std::fs::read_to_string(&config_json).nice_unwrap("Unable to read config.json");
			match serde_json::from_str(config_json_str) {
				Ok(json) => json,
				Err(e) => {
					// Try migrating old config
					// TODO: remove this in 3.0
					let json = serde_json::from_str::<OldConfig>(config_json_str)
						.ok()
						.nice_unwrap(format!("Unable to parse config.json: {}", e));
					info!("Migrating old config.json");
					json.migrate()
				}
			}
		};

		// migrate old profiles from mac to mac-arm or mac-intel
		output.profiles.iter_mut().for_each(|profile| {
			let p = profile.get_mut();
			if p.platform == "mac" {
				p.platform = profile_platform_default();
			}
		});

		output.save();

		if output.profiles.is_empty() {
			warn!("No Sapfire profiles found! Some operations will be unavailable.");
			warn!("Setup Sapfire using `sapfire config setup`");
		} else if output.get_profile(&output.current_profile).is_none() {
			output.current_profile = Some(output.profiles[0].borrow().name.clone());
		}

		output
	}

	pub fn save(&self) {
		std::fs::create_dir_all(sapfire_root()).nice_unwrap("Unable to create Sapfire directory");
		std::fs::write(
			sapfire_root().join("config.json"),
			serde_json::to_string(self).unwrap(),
		)
		.nice_unwrap("Unable to save config");
	}

	pub fn rename_profile(&mut self, old: &str, new: String) {
		let profile = self
			.get_profile(&Some(String::from(old)))
			.nice_unwrap(&format!("Profile named '{}' does not exist", old));

		if self.get_profile(&Some(new.to_owned())).is_some() {
			fail!("The name '{}' is already taken!", new);
		} else {
			done!("Successfully renamed '{}' to '{}'", old, &new);
			profile.borrow_mut().name = new;
		}
	}
}
