export const environmentLabel = "APTOS TESTNET · INTEGRATION ONLY";

export const projectionStates = Object.freeze(["simulated", "recorded", "allocated", "pending", "held", "paid"]);
const catalogue = Object.freeze([
  { id: "demo-after-rain", title: "After the rain", artist: "Morrow", state: "simulated" },
  { id: "demo-soft-architecture", title: "Soft architecture", artist: "Form & Field", state: "recorded" },
  { id: "demo-blue-hours", title: "Blue hours", artist: "Low Tide", state: "allocated" }
]);

export function projectionCopy(state) {
  return {
    simulated: "Illustrative local state. It is not a protocol record.",
    recorded: "Indexed projection of a recorded event. Awaiting any applicable allocation.",
    allocated: "Indexed allocation projection. This is not a payout completion record.",
    pending: "An unpaid obligation may exist on chain. Verify against Move state before relying on it.",
    held: "The projection reports a hold. No payout completion is implied.",
    paid: "Projection reports a completed on-chain payout. Verify the Move event and state independently."
  }[state] ?? "Unknown projection state.";
}

export function canRequestWalletTransaction(contractPackageReady) { return Boolean(contractPackageReady); }

function render(app) {
  const cards = catalogue.map((release) => `<article class="record"><div class="cover" aria-hidden="true">P</div><p class="eyebrow">${release.state}</p><h2>${release.title}</h2><p>${release.artist}</p><p>${projectionCopy(release.state)}</p><button class="secondary" type="button" data-play="${release.id}">Play preview</button></article>`).join("");
  app.innerHTML = `<div class="shell"><header class="topbar"><div><div class="wordmark">porto</div><span class="eyebrow">Made To Be Listened To.</span></div><div class="row"><span class="status">${environmentLabel}</span><button id="wallet" class="secondary" type="button">Connect wallet</button></div></header><section class="hero"><p class="eyebrow">London 0.1.0 dapp shell</p><h1>Music, with the state made clear.</h1><p>Browse the supplied-prototype-inspired catalogue and inspect explicit simulated and indexed protocol projection states. Playback is deliberately user initiated. It does not record a listen, create a usage batch, or submit a transaction.</p></section><section class="grid" aria-label="Porto dapp views"><div class="panel"><p class="eyebrow">Catalogue and playback</p><div class="cards">${cards}</div></div><aside class="panel"><p class="eyebrow">Funding and payouts</p><h2>Read-only projection boundary</h2><ul class="list">${projectionStates.map((state) => `<li><strong>${state}</strong><br>${projectionCopy(state)}</li>`).join("")}</ul><p class="notice">No API endpoint, Move entry function, account address, asset, transaction payload, funding action, or payout action is present until the shared contract package is released.</p></aside></section></div>`;
  app.querySelector("#wallet").addEventListener("click", () => { app.querySelector("#wallet").textContent = "Wallet integration awaits contract package"; });
  app.querySelectorAll("[data-play]").forEach((button) => button.addEventListener("click", () => { button.textContent = "Preview selected · simulated"; }));
}

if (typeof document !== "undefined") { const app = document.querySelector("#app"); if (app) render(app); }
