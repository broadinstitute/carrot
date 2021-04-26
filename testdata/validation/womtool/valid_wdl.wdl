version 1.0

workflow myWorkflow {
    input {
        String? person
        Int times = 3
    }

    call myTask {
        input:
            person = person,
            times = times
    }

    output {
        String greeting = myTask.out
    }
}

task myTask {
    input {
        String person = "Kevin"
        Int times
    }
    command <<<
        echo "hello ~{person} ~{times} times"
    >>>
    output {
        String out = read_string(stdout())
    }
}
