version 1.0

task generate_report_file {

    meta {
        description : "Create a Jupyter Notebook report that presents results of a CARROT test run in a way specified by the user.  Adapted from a task by Jonn Smith"
        author : "Kevin Lydon"
    }

    input {
        # Runtime params
        String cpu
        String memory
        String disks
        String docker
        String maxRetries
        String continueOnReturnCode
        String failOnStderr
        String preemptible
        String bootDiskSizeGb

        File notebook_template
    }

    parameter_meta {
        notebook_template : "A Jupyter notebook that will be run with the other supplied parameters as inputs to generate the report"
    }

    String nb_name = "report.ipynb"
    String html_out = "report.html"

    command <<<
        set -euxo pipefail

        # Copy the notebook template to our current folder:
        cp "~{notebook_template}" ~{nb_name}

        # Do the conversion:

        # Run the notebook and populate the notebook itself:
        jupyter nbconvert --execute ~{nb_name} --to notebook --inplace --no-prompt --no-input --clear-output --debug --ExecutePreprocessor.timeout=7200

        # Convert the notebook output we created just above here to the HTML report:
        jupyter nbconvert ~{nb_name} --to html --no-prompt --no-input --debug --ExecutePreprocessor.timeout=7200
    >>>

    output {
        File populated_notebook = nb_name
        File html_report = html_out
    }

    runtime {
        cpu: cpu
        memory: memory
        disks: disks
        docker: docker
        maxRetries: maxRetries
        continueOnReturnCode: continueOnReturnCode
        failOnStderr: failOnStderr
        preemptible: preemptible
        bootDiskSizeGb: bootDiskSizeGb
    }
}

workflow generate_report_file_workflow {

    meta {
        description : "This workflow generates a Jupyter Notebook from a template to display CARROT run result data.  Adapted from a task by Jonn Smith"
        author : "Kevin Lydon"
    }

    input {
        # Runtime params
        String cpu = "1"
        String memory = "32 GiB"
        String disks = "local-disk 10 SSD"
        String docker
        String maxRetries = "1"
        String continueOnReturnCode = "0"
        String failOnStderr = "false"
        String preemptible = "0"
        String bootDiskSizeGb = "10"

        File notebook_template
    }
    parameter_meta {
        notebook_template : "A Jupyter notebook that will be run with the other supplied parameters as inputs to generate the report"
    }

    call generate_report_file {
        input:
            cpu = cpu,
            memory = memory,
            disks = disks,
            docker = docker,
            maxRetries = maxRetries,
            continueOnReturnCode = continueOnReturnCode,
            failOnStderr = failOnStderr,
            preemptible = preemptible,
            bootDiskSizeGb = bootDiskSizeGb,
            notebook_template = notebook_template,
    }

    output {
        File empty_notebook = notebook_template
        File populated_notebook = generate_report_file.populated_notebook
        File html_report = generate_report_file.html_report
    }
}
