version 1.0

workflow myWorkflow {
    input {
        String person
    }

    call myTask {
        input:
            person = person
    }

    output {
        String greeting = myTask.out
    }
}

task myTask {
    input {
        String person
    }
    command <<<
        echo "hello ~{person}"
    >>>
    output {
        String out = read_string(stdout())
    }
}
