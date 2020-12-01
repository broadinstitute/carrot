version 1.0

task build_and_push {

    input {
        String repo_url
        String software_name
        String commit_hash
        String registry_host
        String github_user
        File github_pass_encrypted
        String gcloud_kms_keyring
        String gcloud_kms_key
    }

    command <<<
        gcloud kms decrypt --location "global" --keyring "~{gcloud_kms_keyring}" --key "~{gcloud_kms_key}" --ciphertext-file ~{github_pass_encrypted} --plaintext-file ./unencrypted.txt
        github_pass=`cat unencrypted.txt`
        git config --global credential.helper store
        echo "https://~{github_user}:$github_pass@github.com" > ~/.git-credentials
        mkdir repo-folder
        cd repo-folder
        git clone ~{repo_url} .
        git checkout ~{commit_hash}
        echo -n "" >> .gcloudignore
        gcloud builds submit --tag ~{registry_host}/~{software_name}:~{commit_hash} --timeout=24h
    >>>

    runtime {
        docker: "google/cloud-sdk:307.0.0"
    }

}

workflow docker_build {

    input {
        String repo_url # The url of the repository containing the project to build from
        String software_name # The name that will be used to name the docker image
        String commit_hash # The hash for the commit to build from; will also be used to tag the image
        String registry_host # The docker repository to push the image to
        String github_user # The username of the github user to authenticate with
        File github_pass_encrypted # The location of the Google Cloud KMS encrypted password or personal access token for github authentication
        String gcloud_kms_keyring # The name of the Google Cloud KMS keyring used to encrypt github_pass_encrypted
        String gcloud_kms_key # The name of the Google Cloud KMS key used to encrypt github_pass_encrypted
    }

    call build_and_push {
        input:
            repo_url = repo_url,
            software_name = software_name,
            commit_hash = commit_hash,
            registry_host = registry_host,
            github_user = github_user,
            github_pass_encrypted = github_pass_encrypted,
            gcloud_kms_keyring = gcloud_kms_keyring,
            gcloud_kms_key = gcloud_kms_key
    }

    output {
        String image_url = registry_host + '/' + software_name + ':' + commit_hash
    }

}
