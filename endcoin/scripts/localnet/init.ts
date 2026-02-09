import { ensureLocalnetInitialized, readLocalnetAddressesFile } from "./common";

async function main() {
  await ensureLocalnetInitialized();
  const summary = readLocalnetAddressesFile();
  if (summary) {
    process.stdout.write(JSON.stringify(summary, null, 2) + "\n");
  }
}

main().catch((err) => {
  console.error(err?.message || err);
  process.exit(1);
});

