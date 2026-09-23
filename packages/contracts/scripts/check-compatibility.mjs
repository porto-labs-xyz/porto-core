import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const fixture = JSON.parse(await readFile(join(root, 'fixtures/london.v1/golden.json'), 'utf8'));
const schema = JSON.parse(await readFile(join(root, 'schemas/london.v1.json'), 'utf8'));
const U64_MAX = 18_446_744_073_709_551_615n;
const isU64 = (value) => typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value) && BigInt(value) <= U64_MAX;
const fail = (message) => { throw new Error(`compatibility failure: ${message}`); };

if (schema.$id !== 'https://contracts.portolabs.xyz/london/v1/schema.json') fail('unexpected schema identity');
if (fixture.schema_version !== 'london.v1') fail('unexpected fixture version');
for (const vector of fixture.integer_money_vectors) {
  if (isU64(vector.value) !== vector.valid) fail(`integer-money vector ${vector.name}`);
}
for (const [name, value] of Object.entries({
  funded_amount: fixture.funding_period.funded_amount,
  rights_version: fixture.rights_snapshot.version,
  sequence_start: fixture.usage_batch.sequence_start,
  sequence_end: fixture.usage_batch.sequence_end,
  aggregate_usage: fixture.usage_batch.aggregate_usage,
  ...fixture.protocol_config.limits,
  ...Object.fromEntries(fixture.rights_snapshot.basis_points.map((value, index) => [`basis_points_${index}`, value]))
})) {
  if (!isU64(value)) fail(`${name} must be a canonical u64 decimal string`);
}
if (fixture.rights_snapshot.beneficiaries.length !== fixture.rights_snapshot.basis_points.length) fail('rights arrays differ in length');
const split = fixture.rights_snapshot.basis_points.reduce((sum, value) => sum + BigInt(value), 0n);
if (split !== 10_000n) fail('rights basis points must total 10000');
if (BigInt(fixture.usage_batch.sequence_start) > BigInt(fixture.usage_batch.sequence_end)) fail('usage range is inverted');
for (const vector of fixture.replay_vectors) {
  const sameRange = vector.first.sequence_start === vector.retry.sequence_start && vector.first.sequence_end === vector.retry.sequence_end;
  const sameDigest = vector.first.payload_digest === vector.retry.payload_digest;
  const overlaps = BigInt(vector.first.sequence_start) <= BigInt(vector.retry.sequence_end) && BigInt(vector.retry.sequence_start) <= BigInt(vector.first.sequence_end);
  const actual = sameRange && sameDigest ? 'idempotent' : overlaps ? 'reject_overlap' : 'accept';
  if (actual !== vector.expected && !(sameRange && !sameDigest && vector.expected === 'reject_conflict')) fail(`replay vector ${vector.name}`);
}
console.log('london.v1 compatibility: PASS (fixture/schema boundary only)');
