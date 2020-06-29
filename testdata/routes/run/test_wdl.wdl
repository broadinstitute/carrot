task print_greeting {
    String greeting
    String greeted
    command {
        echo "${greeting}, ${greeted}"
    }
    output {
        String printed_greeting = read_string(stdout())
    }
}

workflow greeting_workflow {
    String? in_greeting = "Hello"
    String in_greeted
    call print_greeting {
        input:
            greeting = in_greeting,
            greeted = in_greeted
    }
    output {
        String out_greeting = print_greeting.printed_greeting
    }
}