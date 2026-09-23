import test from "node:test";
import assert from "node:assert/strict";
import { canRequestWalletTransaction, environmentLabel, projectionCopy, projectionStates } from "./main.js";

test("labels the experience as testnet integration only", () => { assert.match(environmentLabel, /TESTNET/); assert.match(environmentLabel, /INTEGRATION ONLY/); });
test("documents every explicit projection state", () => { assert.deepEqual(projectionStates, ["simulated", "recorded", "allocated", "pending", "held", "paid"]); for (const state of projectionStates) assert.notEqual(projectionCopy(state), "Unknown projection state."); });
test("blocks wallet transaction requests until the shared contract package is ready", () => { assert.equal(canRequestWalletTransaction(false), false); assert.equal(canRequestWalletTransaction(true), true); });
