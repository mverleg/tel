use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompilationStep {
    Read(ReadId),
    Parse(ParseId),
    Resolve(ResolveId),
    Exec(ExecId),
}

pub struct CompilationLog {
    steps: Vec<CompilationStep>,
}

impl CompilationLog {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn log_read(&mut self, file_path: impl Into<String>) {
        let my_id = ReadId {
            file_path: file_path.into(),
        };
        println!("[qcompiler2] Read: {}", serde_json::to_string(&my_id).unwrap());
        self.steps.push(CompilationStep::Read(my_id));
    }

    pub fn log_parse(&mut self, file_path: impl Into<String>) {
        let my_id = ParseId {
            file_path: file_path.into(),
        };
        println!("[qcompiler2] Parse: {}", serde_json::to_string(&my_id).unwrap());
        self.steps.push(CompilationStep::Parse(my_id));
    }

    pub fn log_resolve(&mut self, func_name: impl Into<String>) {
        let my_id = ResolveId {
            func_name: func_name.into(),
        };
        println!("[qcompiler2] Resolve: {}", serde_json::to_string(&my_id).unwrap());
        self.steps.push(CompilationStep::Resolve(my_id));
    }

    pub fn log_exec(&mut self, main_func: impl Into<String>) {
        let my_id = ExecId {
            main_func: main_func.into(),
        };
        println!("[qcompiler2] Exec: {}", serde_json::to_string(&my_id).unwrap());
        self.steps.push(CompilationStep::Exec(my_id));
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.steps).unwrap()
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
        my_log.log_read("main.telsb");
        my_log.log_parse("main.telsb");
        my_log.log_resolve("main");
        my_log.log_exec("main");

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
