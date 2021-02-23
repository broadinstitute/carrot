version 1.0

task generate_report_file {

    meta {
        description : "Create a Jupyter Notebook report that presents results of a CARROT test run in a way specified by the user.  Adapted from a task by Jonn Smith"
        author : "Kevin Lydon"
    }

    input {
        File notebook_template

        String report_name

[~task_inputs~]
    }

    parameter_meta {
        notebook_template : "A Jupyter notebook that will be run with the other supplied parameters as inputs to generate the report"

        report_name : "The base name to use for the report files, and to include in the metadata section of the report"
    }

    # Determine the disk size based on the files we're using
    Int disk_size = 20 + 8*ceil((
            size(notebook_template, "GB") +
[~input_sizes~]
        ))

    String nb_name = "report.ipynb"
    String html_out = "report.html"
    String pdf_out = "report.pdf"

    command <<<
        set -euxo pipefail

        # Copy the notebook template to our current folder:
        cp "~{notebook_template}" ~{nb_name}

        # Prepare the input file:
        rm -f inputs.config
        echo '{"metadata":{"report_name":"~{report_name}"},"sections":{' >> inputs.config
[~inputs_json~]
        echo '}}' >> inputs.config

        # Do the conversion:

        # Run the notebook and populate the notebook itself:
        jupyter nbconvert --execute ~{nb_name} --to notebook --inplace --no-prompt --no-input --clear-output --debug --ExecutePreprocessor.timeout=7200

        # Convert the notebook output we created just above here to the HTML report:
        jupyter nbconvert ~{nb_name} --to html --no-prompt --no-input --debug --ExecutePreprocessor.timeout=7200

        # One more for good measure - make a PDF so we don't need to wait for the browser all the time.
        jupyter nbconvert ~{nb_name} --to pdf --no-prompt --no-input --debug --ExecutePreprocessor.timeout=7200
    >>>

    output {
        File populated_notebook = nb_name
        File html_report = html_out
        File pdf_report = pdf_out
    }

    runtime {
        cpu: 1
        memory: "64 GiB"
        disks: "local-disk " + disk_size + " HDD"
        bootDiskSizeGb: 10
        preemptible: 0
        maxRetries: 1
        docker: "us.gcr.io/dsde-methods-carrot/jupyter-vis:test"
    }
}

workflow generate_report_file_workflow {

    meta {
        description : "This workflow generates a Jupyter Notebook from a template to display CARROT run result data.  Adapted from a task by Jonn Smith"
        author : "Kevin Lydon"
    }

    input {
        File notebook_template

        String report_name

[~workflow_inputs~]
    }
    parameter_meta {
        notebook_template : "A Jupyter notebook that will be run with the other supplied parameters as inputs to generate the report"

        report_name : "The base name to use for the report files, and to include in the metadata section of the report"
    }

    call generate_report_file {
        input:
            notebook_template = notebook_template,
            report_name = report_name,
[~call_inputs~]
    }

    output {
        File populated_notebook = generate_report_file.populated_notebook
        File html_report = generate_report_file.html_report
        File pdf_report = generate_report_file.pdf_report
    }
}
