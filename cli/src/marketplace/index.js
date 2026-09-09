export {
  KernelMarketplaceBridge,
  MemoryBridgeStore,
  // Hosts branch on this to mark an award permanently poisoned rather than
  // retrying it; without the re-export they had to reach into kernel-bridge.js
  // or match on `error.name`.
  PoisonAwardError,
  SqliteBridgeStore,
  canonicalMarketplaceMessage,
  createAwardCommandPlanner,
  signMarketplaceMessage,
  verifyMarketplaceMessage,
} from './kernel-bridge.js';
