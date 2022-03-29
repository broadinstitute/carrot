version 1.0

task compare {
    input {
        File data_file
        String image_to_use
    }
    command {
        example-test compare ${data_file}
    }
    runtime {
        docker: image_to_use
    }
    output {
        File comparison_result = stdout()
    }
}

workflow eval_workflow {
    input {
        File data_file
        String image_to_use
    }
    call compare {
        input:
            data_file = data_file,
            image_to_use = image_to_use
    }
    output {
        File comparison_result = compare.comparison_result
    }
}