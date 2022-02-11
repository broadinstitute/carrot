version 1.0

import "different_valid_wdl_deps.wdl" as dependency

workflow do_work {
    call dependency.print_a_thing {
        input:
            name = "Kevin"
    }
    output {
        File greeting = print_a_thing.greeting
        String printed_greeting = print_a_thing.printed_greeting
    }
}