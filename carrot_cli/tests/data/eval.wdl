version 1.0

import "eval_dep.wdl" as dep

workflow eval_workflow {
    input {
        File data_file
        String image_to_use
    }
    call dep.compare {
        input:
            data_file = data_file,
            image_to_use = image_to_use
    }
    output {
        File comparison_result = compare.comparison_result
    }
}