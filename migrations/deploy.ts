// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.

const anchor = require("@coral-xyz/anchor");

module.exports = async function () {
  // Configure client to use the provider.
  anchor.setProvider(anchor.AnchorProvider.env());

  // Add your deploy script here.
};