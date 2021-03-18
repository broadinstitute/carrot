task write_greeting_to_file {
    String output_filename
    String greeting
    command {
        echo "${greeting}" > "${output_filename}"
    }
    output {
        String printed_greeting = greeting
        File greeting_file = "${output_filename}"
    }
}

workflow greeting_file_workflow {
    String? in_output_filename = "hello.txt"
    String in_greeting
    call write_greeting_to_file {
        input:
            greeting = in_greeting,
            output_filename = in_output_filename
    }
    output {
        String out_greeting = write_greeting_to_file.printed_greeting
        File out_file = write_greeting_to_file.greeting_file
    }
}
