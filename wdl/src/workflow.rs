use crate::call::Call;
use crate::declaration::Declaration;
use crate::output::Output;

use crate::error::ParseWdlError;
use std::fmt;
use std::str::FromStr;
use crate::import::Import;

#[derive(Debug, PartialEq)]
pub struct Workflow {
    pub imports: Vec<Import>,
    pub name: String,
    pub inputs: Vec<Declaration>,
    pub calls: Vec<Call>,
    pub outputs: Vec<Output>,
}

impl FromStr for Workflow {
    type Err = ParseWdlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lines: Vec<&str> = s.split('\n').collect(); // Split string into lines
        let mut brace_depth : u8 = 0; // For keeping track of how nested the current line is in curly braces
        let mut line_index = 0;
        // For tracking each element we plan to parse
        let mut imports : Vec<Import> = Vec::new();
        let mut name: String = String::new();
        let mut inputs : Vec<Declaration> = Vec::new();
        let mut outputs : Vec<Output> = Vec::new();

        let mut line: &str;
        // Parse line-by-line
        while line_index < lines.len() {

            line = lines.get(line_index).unwrap().trim();
            // If brace depth is 0, we're looking at a workflow, import, or an empty line
            if brace_depth == 0 {
                // If this line is an import, process it as an import
                if line.starts_with("import") {
                    let import : Import = match line.parse() {
                        Ok(val) => val,
                        Err(_) => return Err(ParseWdlError)
                    };
                    imports.push(import);
                }
                // If this is a workflow line, extract the name
                else if line.starts_with("workflow") {
                    name = match line.trim().split(' ').collect::<Vec<&str>>().get(1) {
                        Some(val) => String::from(*val),
                        None => return Err(ParseWdlError)
                    };
                }
            }
            //If it's 1, we're inside the workflow definition
            else if brace_depth == 1 {
                // If this line is the start of an input block, parse each line inside it
                if line.starts_with("input") {
                    loop {
                        // Update brace_depth if necessary
                        if line.ends_with("{") {
                            brace_depth += 1;
                        }
                        // If it doesn't end with { and isn't blank, it's gotta have an input
                        else if !line.is_empty() {
                            let input: Declaration = match line.parse() {
                                Ok(val) => val,
                                Err(_) => return Err(ParseWdlError)
                            };
                            inputs.push(input);
                        }
                        if line.ends_with("}") {
                            brace_depth -= 1;
                            // If we're back at 1 depth, we've reached the end of the input
                            // block, so we're done
                            if brace_depth == 1 {
                                break;
                            }
                        }
                        line_index += 1;
                        line = lines.get(line_index).unwrap().trim();
                    }
                }
                // If this line is the start of an output block, parse each line inside it
                else if line.starts_with("output") {
                    loop {
                        // Update brace_depth if necessary
                        if line.ends_with("{") {
                            brace_depth += 1;
                        }
                        // If it doesn't end with { and isn't blank, it's gotta have an output
                        else if !line.is_empty() {
                            let output: Output = match line.parse() {
                                Ok(val) => val,
                                Err(_) => return Err(ParseWdlError)
                            };
                            outputs.push(output);
                        }
                        if line.ends_with("}") {
                            brace_depth -= 1;
                            // If we're back at 1 depth, we've reached the end of the input
                            // block, so we're done
                            if brace_depth == 1 {
                                break;
                            }
                        }
                        line_index += 1;
                        line = lines.get(line_index).unwrap().trim();
                    }
                }
                // If it's not one of the other possibilities, it must be a declaration
                else if !line.is_empty()
                    && !line.starts_with("if")
                    && !line.starts_with("meta")
                    && !line.starts_with("parameter_meta")
                    && !line.starts_with("call")
                    && !line.starts_with("scatter")
                    && !line.starts_with("}")
                {
                    let input: Declaration = match line.parse() {
                        Ok(val) => val,
                        Err(_) => return Err(ParseWdlError)
                    };
                    inputs.push(input);
                }
            }

            // Update brace_depth if necessary
            if line.trim().ends_with("{") {
                brace_depth += 1;
            }
            else if line.trim().ends_with("}") {
                brace_depth -= 1;
                // If we're back at 0 depth, and we've already got the workflow name, we've
                // reached the end of the workflow definition, so we're done
                if brace_depth == 0 && !name.is_empty() {
                    break;
                }
            }

            line_index += 1;
        }

        Ok(
            Workflow {
                imports,
                name,
                inputs,
                calls: Vec::new(),
                outputs,
            }
        )
    }
}

impl fmt::Display for Workflow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let imports_string = (&self.imports)
            .into_iter()
            .map(|import| import.to_string())
            .collect::<Vec<String>>()
            .join("\n");
        let inputs_string = (&self.inputs)
            .into_iter()
            .map(|input| input.to_string())
            .collect::<Vec<String>>()
            .join("\n    ");
        let outputs_string = (&self.outputs)
            .into_iter()
            .map(|output| output.to_string())
            .collect::<Vec<String>>()
            .join("\n    ");
        let calls_string = (&self.calls)
            .into_iter()
            .map(|call| call.to_string())
            .collect::<Vec<String>>()
            .join("\n\n  ");
        write!(
            f,
            "{}\n\nworkflow {} {{\n  input {{\n    {}\n  }}\n\n  {}\n\n  output {{\n    {}\n  }}\n\n}}",
            imports_string, self.name, inputs_string, calls_string, outputs_string
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::workflow::Workflow;
    use crate::import::Import;
    use crate::declaration::Declaration;
    use crate::call::Call;
    use crate::call_input::CallInput;
    use crate::output::Output;


    #[test]
    fn test_parse() {
        let test_str = "import \"http://test.file/for/test\" as test\n\
            import \"http://test.file/for/eval\" as eval\n\
            \n\
            workflow Test_Workflow {\n  \
              input {\n    \
                File test_file\n    \
                Boolean test_boolean = true\n    \
                String? test_string\n    \
                Array[File]+ test_file_array\n  \
              }\n\
            \n  \
              call test.Main_Workflow as testW {\n    \
                input:\n      \
                  input_test_file = test_file,\n      \
                  is_a_test = test_boolean\n  \
              }\n\
            \n  \
              call eval.Workflow as evalW {\n    \
                input:\n      \
                  eval_file = testW.output_file\n  \
              }\n\
            \n  \
              output {\n    \
                File test_output_file = evalW.output_file\n    \
                String test_output_string = evalW.output_string\n  \
              }\n\n\
            }\n\
            \n\
            task Test_Task {\n  \
              File test_task_file\n  \
              String? test_task_string\n  \
              \n\
              ";
    }

    #[test]
    fn test_to_string() {
        let expected_str = "import \"http://test.file/for/test\" as test\n\
            import \"http://test.file/for/eval\" as eval\n\
            \n\
            workflow Test_Workflow {\n  \
              input {\n    \
                File test_file\n    \
                Boolean test_boolean = true\n    \
                String? test_string\n    \
                Array[File]+ test_file_array\n  \
              }\n\
            \n  \
              call test.Main_Workflow as testW {\n    \
                input:\n      \
                  input_test_file = test_file,\n      \
                  is_a_test = test_boolean\n  \
              }\n\
            \n  \
              call eval.Workflow as evalW {\n    \
                input:\n      \
                  eval_file = testW.output_file\n  \
              }\n\
            \n  \
              output {\n    \
                File test_output_file = evalW.output_file\n    \
                String test_output_string = evalW.output_string\n  \
              }\n\n\
            }";

        let test_workflow = Workflow {
            imports: vec![
                Import {
                    uri: String::from("http://test.file/for/test"),
                    name: String::from("test")
                },
                Import {
                    uri: String::from("http://test.file/for/eval"),
                    name: String::from("eval")
                }
            ],
            name: String::from("Test_Workflow"),
            inputs: vec![
                Declaration {
                    declaration_type: String::from("File"),
                    name: String::from("test_file"),
                    default_value: None,
                    is_optional: false,
                    cannot_be_empty: false,
                },
                Declaration {
                    declaration_type: String::from("Boolean"),
                    name: String::from("test_boolean"),
                    default_value: Some(String::from("true")),
                    is_optional: false,
                    cannot_be_empty: false,
                },
                Declaration {
                    declaration_type: String::from("String"),
                    name: String::from("test_string"),
                    default_value: None,
                    is_optional: true,
                    cannot_be_empty: false,
                },
                Declaration {
                    declaration_type: String::from("Array[File]"),
                    name: String::from("test_file_array"),
                    default_value: None,
                    is_optional: false,
                    cannot_be_empty: true,
                },
            ],
            calls: vec![
                Call {
                    name: String::from("test.Main_Workflow"),
                    alias: Some(String::from("testW")),
                    inputs: vec![
                        CallInput {
                            name: String::from("input_test_file"),
                            value: String::from("test_file"),
                        },
                        CallInput {
                            name: String::from("is_a_test"),
                            value: String::from("test_boolean"),
                        }
                    ],
                },
                Call {
                    name: String::from("eval.Workflow"),
                    alias: Some(String::from("evalW")),
                    inputs: vec![
                        CallInput {
                            name: String::from("eval_file"),
                            value: String::from("testW.output_file"),
                        }
                    ],
                }
            ],
            outputs: vec![
                Output {
                    output_type: String::from("File"),
                    name: String::from("test_output_file"),
                    value: String::from("evalW.output_file"),
                },
                Output {
                    output_type: String::from("String"),
                    name: String::from("test_output_string"),
                    value: String::from("evalW.output_string"),
                }
            ],
        };

        let actual_str = test_workflow.to_string();

        assert_eq!(expected_str, actual_str);
    }


}
