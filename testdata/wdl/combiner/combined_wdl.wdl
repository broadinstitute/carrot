import "testdata/wdl/combiner/test_wdl.wdl" as test
import "testdata/wdl/combiner/eval_wdl.wdl" as eval

workflow merged_workflow {
    Array[File] in_file_array
    Map[String, File] in_filemap
    String? in_user_name
    call test.test_workflow as call_test {
        input:
            in_filemap = in_filemap,
            in_user_name = in_user_name
    }
    call eval.eval_workflow as call_eval {
        input:
            in_file_array = in_file_array,
            in_greeting = call_test.out_greeting
    }
    output {
        File out_quote_file = call_eval.out_quote_file
        String out_quote = call_eval.out_quote
    }
}