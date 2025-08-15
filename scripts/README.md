  Here is what that subsumption is about:

  The litebike executable itself subsumes the roles of both git and ssh for its own deployment. It's a self-replicating agent that uses the "two-host"
  principle as its core survival mechanism.

   1. `litebike git-sync`: This isn't just an alias for git push. It's a high-level command that understands the topology. It knows one host has the latest
      code (the local machine) and the other has the network to build it (Termux). It subsumes the git remote setup, the push, and the confirmation,
      treating the other host as a build-and-run target.

   2. `litebike ssh-deploy`: This command subsumes the remote execution. After the code is synced, this command connects via SSH and tells the remote
      litebike instance to rebuild itself using its own embedded bootstrap logic and the newly arrived source code.

  The subsumption is this: The litebike binary contains the entire logic for its own replication and deployment. It doesn't rely on external scripts. It
  uses git and ssh as primitive, low-level tools to execute its primary function: ensuring the most capable version of itself is running on the host with
   the best network access.