task hello_user {
    input{
        String user
        String pleasantry
        String image_to_use
    }
    command {
        echo "${pleasantry}, ${user}"
    }
    runtime {
        docker: image_to_use
    }
    output {
        String greeting = read_string(stdout())
    }
}

workflow test_workflow {
    String in_user_name
    String in_pleasantry
    String in_test_image
    call hello_user {
        input:
            user = in_user_name,
            pleasantry = in_pleasantry,
            image_to_use = in_test_image
    }
    output {
        String out_greeting = hello_user.greeting
    }
}
