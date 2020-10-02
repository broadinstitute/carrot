task quoted_greeting {
    input{
        String user
        String greeting
        String verb
    }
    command {
        echo "\"${greeting},\" ${user} ${verb}."
    }
    runtime {
        docker: "ubuntu:latest"
    }
    output {
        String they_said = read_string(stdout())
    }
}
workflow eval_workflow {
    String in_greeting
    String in_user
    String in_verb
    call quoted_greeting {
        input:
            greeting = in_greeting,
            user = in_user,
            verb = in_verb
    }
    output {
        String out_quote = quoted_greeting.they_said
    }
}
