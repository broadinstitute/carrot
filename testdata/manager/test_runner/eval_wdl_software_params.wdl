task quoted_greeting {
    input{
        String user
        String greeting
        String verb
        String image_to_use
    }
    command {
        echo "\"${greeting},\" ${user} ${verb}."
    }
    runtime {
        docker: image_to_use
    }
    output {
        String they_said = read_string(stdout())
    }
}
workflow eval_workflow {
    String in_greeting
    String in_user
    String in_verb
    String in_eval_image
    call quoted_greeting {
        input:
            greeting = in_greeting,
            user = in_user,
            verb = in_verb,
            image_to_use = in_eval_image
    }
    output {
        String out_quote = quoted_greeting.they_said
    }
}
