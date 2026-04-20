/**
 * Catalog Commands Module
 */

let catalogSvcPromise = null;

async function getCatalogSvc() {
  if (!catalogSvcPromise) {
    catalogSvcPromise = (async () => {
      const { A2AStore } = await import('../a2a/store.js');
      const { createAgentCatalog } = await import('../catalog/agent-catalog.js');
      const store = new A2AStore();
      store.init();
      return createAgentCatalog(store);
    })();
  }
  return catalogSvcPromise;
}

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

export async function execute(action, args, { output, jsonOutput }) {
  const svc = await getCatalogSvc();

  switch (action) {
    case 'publish': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: catalog publish <payloadJson>');
      const result = svc.publishProduct(parseJsonArg(payloadJson, 'payload'));
      return {
        result,
        formatted: `Published product ${result.productId} as ${result.catalogEntryId}`,
      };
    }

    case 'query': {
      const filtersJson = args[0];
      const result = svc.queryProducts(filtersJson ? parseJsonArg(filtersJson, 'filters') : {});
      return formatCatalogQuery(result, { output, jsonOutput });
    }

    case 'spec': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: catalog spec <productId|catalogEntryId>');
      const result = svc.getProductSpec(identifier);
      if (!result) throw new Error(`Catalog product not found: ${identifier}`);
      return formatCatalogSpec(result, { jsonOutput });
    }

    case 'match-agent': {
      const [capabilitiesJson, trustLevel = 'sandbox'] = args;
      if (!capabilitiesJson) {
        throw new Error('Usage: catalog match-agent <agentCapabilitiesJson> [trustLevel]');
      }
      const result = svc.matchAgentToProducts(
        parseJsonArg(capabilitiesJson, 'agentCapabilities'),
        trustLevel,
      );
      return formatAgentMatches(result, { output, jsonOutput });
    }

    case 'match-product': {
      const [productId, agentsJson] = args;
      if (!productId || !agentsJson) {
        throw new Error('Usage: catalog match-product <productId> <availableAgentsJson>');
      }
      const result = svc.matchProductToAgents(
        productId,
        parseJsonArg(agentsJson, 'availableAgents'),
      );
      return formatProductMatches(result, { output, jsonOutput });
    }

    case 'export': {
      const [format = 'json', category, status] = args;
      const result = svc.exportCatalog({
        format,
        category: category || undefined,
        status: status || undefined,
      });
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Catalog export\n` +
              `${'-'.repeat(24)}\n` +
              `Format:      ${result.format}\n` +
              `Entries:     ${result.entries.length}\n` +
              `Exported:    ${result.exportedAt}`,
          };
    }

    default:
      throw new Error(
        `Unknown action: catalog ${action}\n\n` +
          'Available actions:\n' +
          '  publish <payloadJson>                               Publish product to agent catalog\n' +
          '  query [filtersJson]                                 Query catalog products\n' +
          '  spec <productId|catalogEntryId>                     Get product spec\n' +
          '  match-agent <agentCapabilitiesJson> [trustLevel]    Match agent to products\n' +
          '  match-product <productId> <availableAgentsJson>     Match product to agents\n' +
          '  export [format] [category] [status]                 Export catalog',
      );
  }
}

function formatCatalogQuery(result, { output, jsonOutput }) {
  if (jsonOutput) return result;
  if (result.products.length === 0) return { result, formatted: 'No catalog products found.' };
  const formatted = output.table(
    result.products.map((product) => ({
      id: product.id,
      productId: product.product_id,
      name: product.name,
      category: product.category,
      trust: product.min_trust_level,
      status: product.status,
    })),
    [
      { key: 'id', header: 'Catalog ID' },
      { key: 'productId', header: 'Product' },
      { key: 'name', header: 'Name' },
      { key: 'category', header: 'Category' },
      { key: 'trust', header: 'Trust' },
      { key: 'status', header: 'Status' },
    ],
  );
  return { result, formatted };
}

function formatCatalogSpec(result, { jsonOutput }) {
  if (jsonOutput) return result;
  const { entry, spec } = result;
  return {
    result,
    formatted:
      `Catalog spec: ${entry.name}\n` +
      `${'-'.repeat(34)}\n` +
      `Catalog ID:    ${entry.id}\n` +
      `Product ID:    ${entry.product_id}\n` +
      `Category:      ${entry.category || 'N/A'}\n` +
      `Trust:         ${entry.min_trust_level}\n` +
      `Capabilities:  ${(entry.capabilities || []).join(', ') || 'N/A'}\n` +
      `Required keys: ${(spec.schema?.required || []).join(', ') || 'none'}`,
  };
}

function formatAgentMatches(result, { output, jsonOutput }) {
  if (jsonOutput) return result;
  if (result.compatibleProducts.length === 0)
    return { result, formatted: 'No compatible products found.' };
  const formatted = output.table(
    result.compatibleProducts.map((product) => ({
      id: product.id,
      productId: product.product_id,
      name: product.name,
      matchScore: product.matchScore,
      trust: product.min_trust_level,
    })),
    [
      { key: 'id', header: 'Catalog ID' },
      { key: 'productId', header: 'Product' },
      { key: 'name', header: 'Name' },
      { key: 'matchScore', header: 'Score', align: 'right' },
      { key: 'trust', header: 'Trust' },
    ],
  );
  return { result, formatted };
}

function formatProductMatches(result, { output, jsonOutput }) {
  if (jsonOutput) return result;
  if (result.compatibleAgents.length === 0)
    return { result, formatted: 'No compatible agents found.' };
  const formatted = output.table(result.compatibleAgents, [
    { key: 'id', header: 'Agent' },
    { key: 'trustLevel', header: 'Trust' },
    { key: 'capabilities', header: 'Capabilities' },
  ]);
  return { result, formatted };
}

export const metadata = {
  name: 'catalog',
  aliases: ['cat', 'catalogue'],
  description: 'Agent catalog publishing and discovery commands',
  actions: {
    publish: { description: 'Publish product to catalog', args: ['<payloadJson>'] },
    query: { description: 'Query catalog', args: ['[filtersJson]'] },
    spec: { description: 'Get product spec', args: ['<productId|catalogEntryId>'] },
    'match-agent': {
      description: 'Match agent to products',
      args: ['<agentCapabilitiesJson>', '[trustLevel]'],
    },
    'match-product': {
      description: 'Match product to agents',
      args: ['<productId>', '<availableAgentsJson>'],
    },
    export: { description: 'Export catalog', args: ['[format]', '[category]', '[status]'] },
  },
};

export default { execute, metadata };
