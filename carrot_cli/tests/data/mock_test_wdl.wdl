version 1.0

task make_data_file {
    input {
        Float a
        Float b
        String image_to_use
    }
    command {
        example-test make-data ${a} ${b}
    }
    runtime {
        docker: image_to_use
    }
    output {
        File data_file = stdout()
    }
}

workflow test_workflow {
    input {
        Float a
        Float b
        String image_to_use
    }
    call make_data_file {
        input:
            a = a,
            b = b,
            image_to_use = image_to_use
    }
    output {
        File data_file = make_data_file.data_file
    }
}