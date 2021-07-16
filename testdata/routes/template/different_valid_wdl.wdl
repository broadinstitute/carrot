version 1.0

task print_a_thing {
    input {
        String name
    }
    command {
        echo "Hello, ${name}"
    }
    output {
        File greeting = stdout()
        String printed_greeting = read_string(stdout())
    }
}

workflow do_work {
    call print_a_thing {
        input:
            name = "Kevin"
    }
    output {
        File greeting = print_a_thing.greeting
        String printed_greeting = print_a_thing.printed_greeting
    }
}