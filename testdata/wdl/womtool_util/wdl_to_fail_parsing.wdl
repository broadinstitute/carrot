task echo_task {
  String input_string
  command {
    echo ${input_string}
  }
  output {
    String output_string = read_string(stdout())
  }
}

workflow test_workflow {
  String test_string
  File test_file

  call echo_task {
    input:
      input_string = test_string
  }

  output {
    String test_output = echo_task.output_string
  }
