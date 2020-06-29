task quoted_greeting {
    input{
        String user
        File quoted_file
    }
    command {
        echo "\"${user},\" they said."
    }
    runtime {
        docker: "ubuntu:latest"
    }
    output {
        String they_said = read_string(stdout())
        File output_quote_file = quoted_file
    }
}

workflow eval_workflow {
    String? in_greeting
    Array[File] in_file_array
    call quoted_greeting {
        input:
            user = in_greeting,
            hello_file = in_file_array[0]
    }
    output {
        String out_quote = quoted_greeting.they_said
        File out_quote_file = quoted_greeting.output_quote_file
    }
}