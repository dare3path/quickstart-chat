# SpacetimeDB's quickstart-chat, modified for learning purposes.


This is just a modified version of the quickstart-chat example (client+server) from https://github.com/clockworklabs/SpacetimeDB/  
  
It's meant to be used(by myself) with the TLS/mTLS branch from my SpacetimeDB fork located here: https://github.com/dare3path/SpacetimeDB/tree/ssl_for_standalone_server_NOT_in_PR  
  
# Usage:
- start server via `./start` (server+script is in [my fork](https://github.com/dare3path/SpacetimeDB/tree/ssl_for_standalone_server_NOT_in_PR))  
- `./publish` the server module into the server via script from this repo.  
  Ensure that publishing doesn't fail, but if you get permission denied due to not owning the database, you need a new `data` folder(where database is stored), see what `./wipe-db` says.  
- `./run_client` run the client program which does the chatting for this user.  
- `./logs_show` in another terminal to watch the logs for the db, you've to own it (ie. don't logout then login again, you'll have new identity and thus not own db it anymore).  
  

## Credits
Changes made with the help of xAI's Grok3 and Grok2 because I don't know much of how to Rust or SpacetimeDB. So if things seem wrong, or suboptimal, they might just be.  

## License
- Business Source License 1.1 (see `LICENSE.txt`), switches to AGPLv3 on March 27, 2030

