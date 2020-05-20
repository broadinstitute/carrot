use crate::call::Call;
use crate::declaration::Declaration;
use crate::output::Output;

use crate::error::ParseWdlError;
use std::fmt;
use std::str::FromStr;
use crate::import::Import;

pub struct Workflow {
    pub imports: Vec<Import>,
    pub name: String,
    pub inputs: Vec<Declaration>,
    pub calls: Vec<Call>,
    pub outputs: Vec<Output>,
}

impl fmt::Display for Workflow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let imports_string = (&self.imports)
            .into_iter()
            .map(|import| import.to_string())
            .collect::<Vec<String>>()
            .join(",\n");
        let inputs_string = (&self.inputs)
            .into_iter()
            .map(|input| input.to_string())
            .collect::<Vec<String>>()
            .join(",\n    ");
        let outputs_string = (&self.outputs)
            .into_iter()
            .map(|output| output.to_string())
            .collect::<Vec<String>>()
            .join(",\n    ");
        let calls_string = (&self.calls)
            .into_iter()
            .map(|call| call.to_string())
            .collect::<Vec<String>>()
            .join(",\n  ");
        write!(
            f,
            "{}\nworkflow {} {{\n  input: {{\n    {}\n  }}\n\n  {}\n\n  output: {{\n    {}\n  }}\n\n}}",
            imports_string, self.name, inputs_string, outputs_string, calls_string
        )
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_to_string() {
        let expected_str = "workflow test_workflow {\n  \
        input: {\n    \
        ";
    }
}
