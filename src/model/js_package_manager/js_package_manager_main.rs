use super::{pnpm, yarn};
use crate::{file::path_to_content, model::command};
use anyhow::Result;
use codespan::Files;
use json_spanned_value::{self as jsv, spanned};
use std::{fs, path::PathBuf};

pub(super) const METADATA_FILE_NAME: &str = "package.json";
pub(super) const IGNORE_DIR_NAMES: [&str; 5] = ["node_modules", ".git", ".cache", ".next", ".yarn"];
const METADATA_PACKAGE_NAME_KEY: &str = "name";
const METADATA_COMMAND_KEY: &str = "scripts";

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum JsPackageManager {
    JsPnpm(pnpm::Pnpm),
    JsYarn(yarn::Yarn),
}

impl JsPackageManager {
    pub fn command_to_run(&self, command: &command::Command) -> Result<String> {
        match self {
            JsPackageManager::JsPnpm(pnpm) => pnpm.command_to_run(command),
            JsPackageManager::JsYarn(yarn) => yarn.command_to_run(command),
        }
    }

    pub fn to_commands(&self) -> Vec<command::Command> {
        match self {
            JsPackageManager::JsPnpm(pnpm) => pnpm.to_commands(),
            JsPackageManager::JsYarn(yarn) => yarn.to_commands(),
        }
    }

    pub fn execute(&self, command: &command::Command) -> Result<()> {
        match self {
            JsPackageManager::JsPnpm(pnpm) => pnpm.execute(command),
            JsPackageManager::JsYarn(yarn) => yarn.execute(command),
        }
    }

    pub fn path(&self) -> PathBuf {
        match self {
            JsPackageManager::JsPnpm(pnpm) => pnpm.path.clone(),
            JsPackageManager::JsYarn(yarn) => yarn.path.clone(),
        }
    }

    fn new(current_dir: PathBuf, file_names: Vec<String>) -> Option<Self> {
        let metadata_file_path = &PathBuf::from(METADATA_FILE_NAME);

        // TODO: refactor same as yarn
        // Search for lockfile to identify the package manager.
        for file_name in file_names.clone() {
            if pnpm::Pnpm::use_pnpm(&file_name) {
                if let Ok(c) = path_to_content::path_to_content(metadata_file_path) {
                    if let Some(result) = JsPackageManager::parse_package_json(&c) {
                        return Some(JsPackageManager::JsPnpm(pnpm::Pnpm::new(
                            current_dir.clone(),
                            result.1,
                        )));
                    }
                }
                break;
            }
        }

        if let Some(r) = yarn::Yarn::new(current_dir, file_names) {
            return Some(JsPackageManager::JsYarn(r));
        }

        None
    }

    // returns (package_name, [(script_name, script_content, line_number)]
    #[allow(clippy::type_complexity)]
    pub fn parse_package_json(content: &str) -> Option<(String, Vec<(String, String, u32)>)> {
        let mut files = Files::new();
        let file = files.add(METADATA_FILE_NAME, content);
        let json_object: spanned::Object = match jsv::from_str(content) {
            Ok(e) => e,
            Err(_) => return None,
        };

        let mut name = "".to_string();
        let mut result = vec![];
        for (k, v) in json_object {
            if k.as_str() == METADATA_PACKAGE_NAME_KEY && v.as_string().is_some() {
                name = v.as_string().unwrap().to_string();
            }
            if k.as_str() != METADATA_COMMAND_KEY {
                continue;
            }

            // object is the content of "scripts" key
            if let Some(object) = v.as_object() {
                for (k, v) in object {
                    let args = k.to_string();
                    let line_number =
                        files.line_index(file, k.start() as u32).number().to_usize() as u32;
                    if let Some(v) = v.as_string() {
                        result.push((args, v.to_string(), line_number));
                    }
                }
            };
            break;
        }

        Some((name, result))
    }
}

pub fn get_js_package_manager_runner(current_dir: PathBuf) -> Option<JsPackageManager> {
    let entries = fs::read_dir(current_dir.clone()).unwrap();
    let file_names = entries
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();

    JsPackageManager::new(current_dir, file_names)
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_package_json() {
        struct Case {
            title: &'static str,
            file_content: &'static str,
            #[allow(clippy::type_complexity)]
            expected: Option<(String, Vec<(String, String, u32)>)>,
        }

        let cases = vec![
            Case {
                title: "valid json can be parsed successfully",
                file_content: r#"{
      "name": "project",
      "version": "1.0.0",
      "private": true,
      "scripts": {
        "build": "echo build",
        "start": "echo start",
        "test": "echo test"
      },
      "devDependencies": {
        "@babel/cli": "7.12.10"
      },
      "dependencies": {
        "firebase": "^8.6.8"
      }
    }
                        "#,
                expected: Some((
                    "project".to_string(),
                    vec![
                        ("build".to_string(), "echo build".to_string(), 6),
                        ("start".to_string(), "echo start".to_string(), 7),
                        ("test".to_string(), "echo test".to_string(), 8),
                    ],
                )),
            },
            Case {
                title: "empty vec(empty string)",
                file_content: "",
                expected: None,
            },
            Case {
                title: "empty vec(invalid json)",
                file_content: "not a json format",
                expected: None,
            },
        ];

        for case in cases {
            assert_eq!(
                case.expected,
                JsPackageManager::parse_package_json(case.file_content),
                "\nfailed: 🚨{:?}🚨\n",
                case.title,
            );
        }
    }
}
