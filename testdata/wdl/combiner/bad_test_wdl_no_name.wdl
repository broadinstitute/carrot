task hello_user {
    input{
        String user
        File hello_file
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

workflow {
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
    }
}
