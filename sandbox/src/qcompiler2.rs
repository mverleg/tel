use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReadId {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParseId {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResolveId {
    pub func_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecId {
    pub main_func: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepId {
    Read(ReadId),
    Parse(ParseId),
    Resolve(ResolveId),
    Exec(ExecId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub from: StepId,
    pub to: StepId,
}

pub struct CompilationLog {
    call_stack: Vec<StepId>,
    dependencies: Vec<Dependency>,
}

impl CompilationLog {
    pub fn new() -> Self {
        Self {
            call_stack: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    fn enter_step(&mut self, a_step: StepId) {
        println!("[qcompiler2] Enter: {}", serde_json::to_string(&a_step).unwrap());

        if let Some(my_parent) = self.call_stack.last() {
            let my_dep = Dependency {
                from: my_parent.clone(),
                to: a_step.clone(),
            };
            println!("[qcompiler2] Dependency: {} -> {}",
                serde_json::to_string(&my_dep.from).unwrap(),
                serde_json::to_string(&my_dep.to).unwrap());
            self.dependencies.push(my_dep);
        }

        self.call_stack.push(a_step);
    }

    fn exit_step(&mut self) {
        if let Some(my_step) = self.call_stack.pop() {
            println!("[qcompiler2] Exit: {}", serde_json::to_string(&my_step).unwrap());
        }
    }

    pub fn in_read<T>(&mut self, file_path: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let my_id = StepId::Read(ReadId {
            file_path: file_path.into(),
        });
        self.enter_step(my_id);
        let my_result = f(self);
        self.exit_step();
        my_result
    }

    pub fn in_parse<T>(&mut self, file_path: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let my_id = StepId::Parse(ParseId {
            file_path: file_path.into(),
        });
        self.enter_step(my_id);
        let my_result = f(self);
        self.exit_step();
        my_result
    }

    pub fn in_resolve<T>(&mut self, func_name: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let my_id = StepId::Resolve(ResolveId {
            func_name: func_name.into(),
        });
        self.enter_step(my_id);
        let my_result = f(self);
        self.exit_step();
        my_result
    }

    pub fn in_exec<T>(&mut self, main_func: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let my_id = StepId::Exec(ExecId {
            main_func: main_func.into(),
        });
        self.enter_step(my_id);
        let my_result = f(self);
        self.exit_step();
        my_result
    }

    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    pub fn to_json(&self) -> String {
        let mut my_graph: HashMap<String, Vec<String>> = HashMap::new();

        for dep in &self.dependencies {
            let my_from = serde_json::to_string(&dep.from).unwrap();
            let my_to = serde_json::to_string(&dep.to).unwrap();
            my_graph.entry(my_from).or_insert_with(Vec::new).push(my_to);
        }

        serde_json::to_string_pretty(&my_graph).unwrap()
    }
}

impl Default for CompilationLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_id_serialization() {
        let my_id = ReadId {
            file_path: "test.telsb".to_string(),
        };
        let my_json = serde_json::to_string(&my_id).unwrap();
        let my_deserialized: ReadId = serde_json::from_str(&my_json).unwrap();
        assert_eq!(my_id, my_deserialized);
    }

    #[test]
    fn test_compilation_log() {
        let mut my_log = CompilationLog::new();
        my_log.in_read("main.telsb", |log| {
            log.in_parse("main.telsb", |log| {
                log.in_resolve("main", |log| {
                    log.in_exec("main", |_log| {
                    })
                })
            })
        });

        let my_json = my_log.to_json();
        assert!(my_json.contains("main.telsb"));
        assert!(my_json.contains("Read"));
        assert!(my_json.contains("Parse"));
        assert!(my_json.contains("Resolve"));
        assert!(my_json.contains("Exec"));
    }

    #[test]
    fn test_all_id_types_serializable() {
        let my_parse_id = ParseId {
            file_path: "test.telsb".to_string(),
        };
        let my_resolve_id = ResolveId {
            func_name: "my_func".to_string(),
        };
        let my_exec_id = ExecId {
            main_func: "main".to_string(),
        };

        assert!(serde_json::to_string(&my_parse_id).is_ok());
        assert!(serde_json::to_string(&my_resolve_id).is_ok());
        assert!(serde_json::to_string(&my_exec_id).is_ok());
    }
}
