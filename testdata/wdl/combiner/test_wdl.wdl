## Comments for trying to mess with the parser
## workflow
## workflow cool_workflow {
## String out_output
## File in_input

task hello_user {
    input{
        String user
        File hello_file # File in_file
    }
    command {
        echo "Hello ${user}"
    }
    runtime {
        docker: "ubuntu:latest"
    }
    output {
        String greeting = read_string(stdout())
        File output_hello = hello_file
    }
}

workflow test_workflow {
    String? in_user_name
    Map[String, File] in_filemap
    call hello_user {
        input:
            user = in_user_name,
            hello_file = in_filemap["hello"]
    }
    output {
        String out_greeting = hello_user.greeting
        File out_file = hello_user.output_hello
        # String out_string
    }
}
