task hello_user {
    input{
        String user
        String pleasantry
    }
    command {
        echo "${pleasantry}, ${user}"
    }
    runtime {
        docker: "ubuntu:latest"
    }
    output {
        String greeting = read_string(stdout())
    }
}

workflow test_workflow {
    String in_user_name
    String in_pleasantry
    call hello_user {
        input:
            user = in_user_name,
            pleasantry = in_pleasantry
    }
    output {
        String out_greeting = hello_user.greeting
    }
}
