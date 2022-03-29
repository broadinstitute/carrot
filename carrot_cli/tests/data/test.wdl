version 1.0

import "test_dep.wdl" as dep

workflow test_workflow {
    input {
        Float a
        Float b
        String image_to_use
    }
    call dep.make_data_file {
        input:
            a = a,
            b = b,
            image_to_use = image_to_use
    }
    output {
        File data_file = make_data_file.data_file
    }
}