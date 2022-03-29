import "valid_wdl_deps.wdl" as dependency

workflow myWorkflow {
    call dependency.myTask
}
