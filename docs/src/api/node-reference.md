# `@stateset/embedded` API reference

<!-- GENERATED FILE. Do not edit by hand: run `node scripts/generate-api-reference.mjs` in bindings/node. -->

Generated from `bindings/node/index.d.ts` by `bindings/node/scripts/generate-api-reference.mjs`.
The declarations are themselves generated from the Rust binding, so this page is the
authoritative list of what the package exports. For guided examples see [Node.js](node.md).

All monetary fields are exact base-10 decimal strings (field names ending in `Exact`);
float twins, where they still exist, are deprecated. Methods return Promises unless noted.

## Contents

- [Commerce](#commerce)
- [Sub-APIs](#sub-apis)
  - [`commerce.customers`](#commercecustomers) — `Customers`
  - [`commerce.orders`](#commerceorders) — `Orders`
  - [`commerce.products`](#commerceproducts) — `Products`
  - [`commerce.customObjects`](#commercecustomobjects) — `CustomObjects`
  - [`commerce.inventory`](#commerceinventory) — `Inventory`
  - [`commerce.returns`](#commercereturns) — `Returns`
  - [`commerce.giftCards`](#commercegiftcards) — `GiftCards`
  - [`commerce.loyalty`](#commerceloyalty) — `Loyalty`
  - [`commerce.storeCredits`](#commercestorecredits) — `StoreCredits`
  - [`commerce.reviews`](#commercereviews) — `Reviews`
  - [`commerce.wishlists`](#commercewishlists) — `Wishlists`
  - [`commerce.segments`](#commercesegments) — `Segments`
  - [`commerce.payments`](#commercepayments) — `Payments`
  - [`commerce.x402`](#commercex402) — `X402`
  - [`commerce.shipments`](#commerceshipments) — `Shipments`
  - [`commerce.warranties`](#commercewarranties) — `Warranties`
  - [`commerce.purchaseOrders`](#commercepurchaseorders) — `PurchaseOrders`
  - [`commerce.invoices`](#commerceinvoices) — `Invoices`
  - [`commerce.bom`](#commercebom) — `Bom`
  - [`commerce.workOrders`](#commerceworkorders) — `WorkOrders`
  - [`commerce.carts`](#commercecarts) — `Carts`
  - [`commerce.analytics`](#commerceanalytics) — `Analytics`
  - [`commerce.currency`](#commercecurrency) — `CurrencyOperations`
  - [`commerce.subscriptions`](#commercesubscriptions) — `Subscriptions`
  - [`commerce.promotions`](#commercepromotions) — `Promotions`
  - [`commerce.tax`](#commercetax) — `Tax`
  - [`commerce.quality`](#commercequality) — `Quality`
  - [`commerce.lots`](#commercelots) — `Lots`
  - [`commerce.serials`](#commerceserials) — `Serials`
  - [`commerce.warehouse`](#commercewarehouse) — `Warehouse`
  - [`commerce.receiving`](#commercereceiving) — `Receiving`
  - [`commerce.fulfillment`](#commercefulfillment) — `Fulfillment`
  - [`commerce.accountsPayable`](#commerceaccountspayable) — `AccountsPayable`
  - [`commerce.accountsReceivable`](#commerceaccountsreceivable) — `AccountsReceivable`
  - [`commerce.costAccounting`](#commercecostaccounting) — `CostAccounting`
  - [`commerce.credit`](#commercecredit) — `Credit`
  - [`commerce.backorder`](#commercebackorder) — `Backorders`
  - [`commerce.generalLedger`](#commercegeneralledger) — `GeneralLedger`
  - [`commerce.fixedAssets`](#commercefixedassets) — `FixedAssets`
  - [`commerce.revenueRecognition`](#commercerevenuerecognition) — `RevenueRecognition`
  - [`commerce.cycleCounts`](#commercecyclecounts) — `CycleCounts`
  - [`commerce.ediDocuments`](#commerceedidocuments) — `EdiDocuments`
  - [`commerce.activityLogs`](#commerceactivitylogs) — `ActivityLogs`
  - [`commerce.channels`](#commercechannels) — `Channels`
  - [`commerce.companies`](#commercecompanies) — `Companies`
  - [`commerce.unitsOfMeasure`](#commerceunitsofmeasure) — `UnitsOfMeasure`
  - [`commerce.shippingZones`](#commerceshippingzones) — `ShippingZones`
  - [`commerce.stockSnapshots`](#commercestocksnapshots) — `StockSnapshots`
  - [`commerce.printStations`](#commerceprintstations) — `PrintStations`
  - [`commerce.integrationMappings`](#commerceintegrationmappings) — `IntegrationMappings`
  - [`commerce.integrationFieldMappings`](#commerceintegrationfieldmappings) — `IntegrationFieldMappings`
  - [`commerce.paymentObligations`](#commercepaymentobligations) — `PaymentObligations`
  - [`commerce.maintenance`](#commercemaintenance) — `Maintenance`
  - [`commerce.purgatory`](#commercepurgatory) — `Purgatory`
  - [`commerce.topologySnapshots`](#commercetopologysnapshots) — `TopologySnapshots`
  - [`commerce.fraud`](#commercefraud) — `Fraud`
  - [`commerce.searchConfig`](#commercesearchconfig) — `SearchConfigs`
  - [`commerce.erc8004`](#commerceerc8004) — `Erc8004`
  - [`commerce.vendorReturns`](#commercevendorreturns) — `VendorReturns`
  - [`commerce.prepayments`](#commerceprepayments) — `Prepayments`
  - [`commerce.vendorCredits`](#commercevendorcredits) — `VendorCredits`
  - [`commerce.priceSchedules`](#commercepriceschedules) — `PriceSchedules`
  - [`commerce.priceLevels`](#commercepricelevels) — `PriceLevels`
  - [`commerce.transferOrders`](#commercetransferorders) — `TransferOrders`
  - [`commerce.productionBatches`](#commerceproductionbatches) — `ProductionBatches`
  - [`commerce.supplierSkus`](#commercesupplierskus) — `SupplierSkus`
  - [`commerce.inboundShipments`](#commerceinboundshipments) — `InboundShipments`
  - [`commerce.events`](#commerceevents) — `Events`
  - [`commerce.vector(apiKey)`](#commercevectorapikey) — `VectorSearch`
  - [`commerce.events.subscribe()`](#commerceeventssubscribe) — `CommerceEventSubscription`
- [Functions](#functions)
  - [`vesStrictGenerateSigningKeypair`](#vesstrictgeneratesigningkeypair)
  - [`vesStrictSignEventHash`](#vesstrictsigneventhash)
  - [`vesStrictVerifyEventSignature`](#vesstrictverifyeventsignature)
  - [`vesStrictGenerateRecipientKeypair`](#vesstrictgeneraterecipientkeypair)
  - [`vesStrictEncryptPayload`](#vesstrictencryptpayload)
  - [`vesStrictDecryptPayload`](#vesstrictdecryptpayload)
  - [`vesHybridGenerateSigningPop`](#veshybridgeneratesigningpop)
  - [`vesHybridVerifySigningPop`](#veshybridverifysigningpop)
  - [`vesStrictGenerateSigningPop`](#vesstrictgeneratesigningpop)
  - [`vesStrictVerifySigningPop`](#vesstrictverifysigningpop)
  - [`aesGcmEncrypt`](#aesgcmencrypt)
  - [`aesGcmDecrypt`](#aesgcmdecrypt)
  - [`merkleRoot`](#merkleroot)
  - [`jcsCanonicalize`](#jcscanonicalize)
  - [`domainHash`](#domainhash)
  - [`ed25519Sign`](#ed25519sign)
  - [`ed25519Verify`](#ed25519verify)
  - [`vesHybridGenerateSigningKeypair`](#veshybridgeneratesigningkeypair)
  - [`vesHybridSignEventHash`](#veshybridsigneventhash)
  - [`vesHybridVerifyEventSignature`](#veshybridverifyeventsignature)
  - [`vesTestVectorMlDsaPublicKey`](#vestestvectormldsapublickey)
  - [`vesHybridGenerateRecipientKeypair`](#veshybridgeneraterecipientkeypair)
  - [`vesTestVectorMlKemPublicKey`](#vestestvectormlkempublickey)
  - [`vesHybridEncryptPayload`](#veshybridencryptpayload)
  - [`vesHybridDecryptPayload`](#veshybriddecryptpayload)
  - [`vesX402ComputeSigningHash`](#vesx402computesigninghash)
- [Types](#types)
  - [`ActivityLogEntryOutput`](#activitylogentryoutput)
  - [`ActivityLogFilterInput`](#activitylogfilterinput)
  - [`ActorKind`](#actorkind)
  - [`AddCartItemExactInput`](#addcartitemexactinput)
  - [`AddCartItemInput`](#addcartiteminput)
  - [`AddWishlistItemInput`](#addwishlistiteminput)
  - [`AddressType`](#addresstype)
  - [`AdjustPointsInput`](#adjustpointsinput)
  - [`AdjustStoreCreditInput`](#adjuststorecreditinput)
  - [`AgentFeedbackFilterInput`](#agentfeedbackfilterinput)
  - [`AgentFeedbackOutput`](#agentfeedbackoutput)
  - [`AgentIdentityFilterInput`](#agentidentityfilterinput)
  - [`AgentIdentityOutput`](#agentidentityoutput)
  - [`AgentValidationRequestOutput`](#agentvalidationrequestoutput)
  - [`AgentValidationResponseOutput`](#agentvalidationresponseoutput)
  - [`AgentValidationStatusOutput`](#agentvalidationstatusoutput)
  - [`AgentWalletProofInput`](#agentwalletproofinput)
  - [`AgentWalletProofType`](#agentwalletprooftype)
  - [`AgentWalletProofTypeInput`](#agentwalletprooftypeinput)
  - [`AnalyticsGranularity`](#analyticsgranularity)
  - [`AnalyticsPeriod`](#analyticsperiod)
  - [`AnalyticsQueryInput`](#analyticsqueryinput)
  - [`ApAgingSummaryOutput`](#apagingsummaryoutput)
  - [`AppliedPromotionOutput`](#appliedpromotionoutput)
  - [`ApplyPrepaymentInput`](#applyprepaymentinput)
  - [`ApplyPromotionsInput`](#applypromotionsinput)
  - [`ApplyPromotionsOutput`](#applypromotionsoutput)
  - [`ApplyVendorCreditInput`](#applyvendorcreditinput)
  - [`ArAgingSummaryOutput`](#aragingsummaryoutput)
  - [`AssetAmountWire`](#assetamountwire)
  - [`AssetDisposalOutput`](#assetdisposaloutput)
  - [`BackorderFilterInput`](#backorderfilterinput)
  - [`BackorderOutput`](#backorderoutput)
  - [`BackorderPriority`](#backorderpriority)
  - [`BackorderPriorityFilter`](#backorderpriorityfilter)
  - [`BackorderPriorityInput`](#backorderpriorityinput)
  - [`BackorderStatus`](#backorderstatus)
  - [`BackorderStatusInput`](#backorderstatusinput)
  - [`BackorderSummaryOutput`](#backordersummaryoutput)
  - [`BackupManifestOutput`](#backupmanifestoutput)
  - [`BackupReportOutput`](#backupreportoutput)
  - [`BalanceSheetOutput`](#balancesheetoutput)
  - [`BillFilterInput`](#billfilterinput)
  - [`BillOutput`](#billoutput)
  - [`BillStatus`](#billstatus)
  - [`BillStatusInput`](#billstatusinput)
  - [`BillingCycleFilterInput`](#billingcyclefilterinput)
  - [`BillingCycleOutput`](#billingcycleoutput)
  - [`BillingCycleStatus`](#billingcyclestatus)
  - [`BomComponentOutput`](#bomcomponentoutput)
  - [`BomFilterInput`](#bomfilterinput)
  - [`BomOutput`](#bomoutput)
  - [`BomStatus`](#bomstatus)
  - [`BoostRuleInput`](#boostruleinput)
  - [`BoostRuleOutput`](#boostruleoutput)
  - [`BulkSupplierSkuItemInput`](#bulksupplierskuiteminput)
  - [`CanadianTaxInfoOutput`](#canadiantaxinfooutput)
  - [`CancelSubscriptionInput`](#cancelsubscriptioninput)
  - [`CaptureStockLineInput`](#capturestocklineinput)
  - [`CaptureStockSnapshotInput`](#capturestocksnapshotinput)
  - [`CaptureTopologySnapshotInput`](#capturetopologysnapshotinput)
  - [`CartAddressInput`](#cartaddressinput)
  - [`CartAddressOutput`](#cartaddressoutput)
  - [`CartAddressSnapshot`](#cartaddresssnapshot)
  - [`CartFilterInput`](#cartfilterinput)
  - [`CartItemOutput`](#cartitemoutput)
  - [`CartItemSnapshot`](#cartitemsnapshot)
  - [`CartOutput`](#cartoutput)
  - [`CartPaymentStatus`](#cartpaymentstatus)
  - [`CartSnapshot`](#cartsnapshot)
  - [`CartStatus`](#cartstatus)
  - [`CartX402PaymentSnapshot`](#cartx402paymentsnapshot)
  - [`ChannelFilterInput`](#channelfilterinput)
  - [`ChannelOutput`](#channeloutput)
  - [`ChannelProductMappingOutput`](#channelproductmappingoutput)
  - [`ChannelProductSyncItemInput`](#channelproductsynciteminput)
  - [`ChannelStatus`](#channelstatus)
  - [`ChannelType`](#channeltype)
  - [`CheckoutResultOutput`](#checkoutresultoutput)
  - [`CheckoutSnapshot`](#checkoutsnapshot)
  - [`CloseMonthOptionsInput`](#closemonthoptionsinput)
  - [`CloseMonthReportOutput`](#closemonthreportoutput)
  - [`CloseMonthStepOutput`](#closemonthstepoutput)
  - [`CloseMonthStepStatus`](#closemonthstepstatus)
  - [`CommerceEvent`](#commerceevent)
  - [`CompanyFilterInput`](#companyfilterinput)
  - [`CompanyOutput`](#companyoutput)
  - [`CompanyPriceOverrideOutput`](#companypriceoverrideoutput)
  - [`CompanyShippingAddressOutput`](#companyshippingaddressoutput)
  - [`CompanyStatus`](#companystatus)
  - [`ContactOutput`](#contactoutput)
  - [`ConversionResultOutput`](#conversionresultoutput)
  - [`ConversionRuleType`](#conversionruletype)
  - [`ConvertCurrencyInput`](#convertcurrencyinput)
  - [`CostMethod`](#costmethod)
  - [`CostMethodFilter`](#costmethodfilter)
  - [`CostMethodInput`](#costmethodinput)
  - [`CouponFilterInput`](#couponfilterinput)
  - [`CouponOutput`](#couponoutput)
  - [`CouponStatus`](#couponstatus)
  - [`CreateAgentFeedbackInput`](#createagentfeedbackinput)
  - [`CreateAgentIdentityInput`](#createagentidentityinput)
  - [`CreateAgentValidationRequestInput`](#createagentvalidationrequestinput)
  - [`CreateAgentValidationResponseInput`](#createagentvalidationresponseinput)
  - [`CreateBackorderInput`](#createbackorderinput)
  - [`CreateBillInput`](#createbillinput)
  - [`CreateBomComponentInput`](#createbomcomponentinput)
  - [`CreateBomInput`](#createbominput)
  - [`CreateCartInput`](#createcartinput)
  - [`CreateChannelInput`](#createchannelinput)
  - [`CreateCompanyInput`](#createcompanyinput)
  - [`CreateContactInput`](#createcontactinput)
  - [`CreateCouponInput`](#createcouponinput)
  - [`CreateCreditAccountInput`](#createcreditaccountinput)
  - [`CreateCreditMemoInput`](#createcreditmemoinput)
  - [`CreateCustomObjectInput`](#createcustomobjectinput)
  - [`CreateCustomObjectTypeInput`](#createcustomobjecttypeinput)
  - [`CreateCustomerAddressInput`](#createcustomeraddressinput)
  - [`CreateCustomerInput`](#createcustomerinput)
  - [`CreateCycleCountInput`](#createcyclecountinput)
  - [`CreateCycleCountLineInput`](#createcyclecountlineinput)
  - [`CreateEdiDocumentInput`](#createedidocumentinput)
  - [`CreateExemptionInput`](#createexemptioninput)
  - [`CreateFixedAssetInput`](#createfixedassetinput)
  - [`CreateFraudAssessmentInput`](#createfraudassessmentinput)
  - [`CreateFraudRuleInput`](#createfraudruleinput)
  - [`CreateFraudSignalInput`](#createfraudsignalinput)
  - [`CreateGiftCardInput`](#creategiftcardinput)
  - [`CreateGlAccountInput`](#createglaccountinput)
  - [`CreateGlPeriodInput`](#createglperiodinput)
  - [`CreateInboundShipmentInput`](#createinboundshipmentinput)
  - [`CreateInboundShipmentItemInput`](#createinboundshipmentiteminput)
  - [`CreateInspectionInput`](#createinspectioninput)
  - [`CreateIntegrationFieldMappingInput`](#createintegrationfieldmappinginput)
  - [`CreateIntegrationMappingInput`](#createintegrationmappinginput)
  - [`CreateInventoryItemInput`](#createinventoryiteminput)
  - [`CreateInvoiceInput`](#createinvoiceinput)
  - [`CreateInvoiceItemInput`](#createinvoiceiteminput)
  - [`CreateJurisdictionInput`](#createjurisdictioninput)
  - [`CreateLocationInput`](#createlocationinput)
  - [`CreateLotInput`](#createlotinput)
  - [`CreateLoyaltyProgramInput`](#createloyaltyprograminput)
  - [`CreateNcrInput`](#createncrinput)
  - [`CreateOrderExactInput`](#createorderexactinput)
  - [`CreateOrderInput`](#createorderinput)
  - [`CreateOrderItemExactInput`](#createorderitemexactinput)
  - [`CreateOrderItemInput`](#createorderiteminput)
  - [`CreatePaymentExactInput`](#createpaymentexactinput)
  - [`CreatePaymentInput`](#createpaymentinput)
  - [`CreatePaymentObligationInput`](#createpaymentobligationinput)
  - [`CreatePerformanceObligationInput`](#createperformanceobligationinput)
  - [`CreatePrepaymentInput`](#createprepaymentinput)
  - [`CreatePriceLevelInput`](#createpricelevelinput)
  - [`CreatePriceScheduleInput`](#createpricescheduleinput)
  - [`CreatePrintStationInput`](#createprintstationinput)
  - [`CreateProductInput`](#createproductinput)
  - [`CreateProductVariantInput`](#createproductvariantinput)
  - [`CreateProductionBatchInput`](#createproductionbatchinput)
  - [`CreatePromotionInput`](#createpromotioninput)
  - [`CreatePurchaseOrderInput`](#createpurchaseorderinput)
  - [`CreatePurchaseOrderItemInput`](#createpurchaseorderiteminput)
  - [`CreateQualityHoldInput`](#createqualityholdinput)
  - [`CreateReceiptInput`](#createreceiptinput)
  - [`CreateRefundExactInput`](#createrefundexactinput)
  - [`CreateRefundInput`](#createrefundinput)
  - [`CreateReturnInput`](#createreturninput)
  - [`CreateReturnItemInput`](#createreturniteminput)
  - [`CreateRevenueContractInput`](#createrevenuecontractinput)
  - [`CreateReviewInput`](#createreviewinput)
  - [`CreateRewardInput`](#createrewardinput)
  - [`CreateSearchConfigInput`](#createsearchconfiginput)
  - [`CreateSegmentInput`](#createsegmentinput)
  - [`CreateSerialInput`](#createserialinput)
  - [`CreateShipmentInput`](#createshipmentinput)
  - [`CreateShippingZoneInput`](#createshippingzoneinput)
  - [`CreateStoreCreditInput`](#createstorecreditinput)
  - [`CreateSubscriptionInput`](#createsubscriptioninput)
  - [`CreateSubscriptionPlanInput`](#createsubscriptionplaninput)
  - [`CreateSupplierInput`](#createsupplierinput)
  - [`CreateSupplierSkuInput`](#createsupplierskuinput)
  - [`CreateTaxRateInput`](#createtaxrateinput)
  - [`CreateTransferOrderInput`](#createtransferorderinput)
  - [`CreateTransferOrderItemInput`](#createtransferorderiteminput)
  - [`CreateUnitClassInput`](#createunitclassinput)
  - [`CreateUnitConversionRuleInput`](#createunitconversionruleinput)
  - [`CreateUnitOfMeasureInput`](#createunitofmeasureinput)
  - [`CreateVendorCreditInput`](#createvendorcreditinput)
  - [`CreateVendorReturnInput`](#createvendorreturninput)
  - [`CreateVendorReturnItemInput`](#createvendorreturniteminput)
  - [`CreateWarehouseInput`](#createwarehouseinput)
  - [`CreateWarrantyClaimInput`](#createwarrantyclaiminput)
  - [`CreateWarrantyInput`](#createwarrantyinput)
  - [`CreateWaveInput`](#createwaveinput)
  - [`CreateWebhookInput`](#createwebhookinput)
  - [`CreateWishlistInput`](#createwishlistinput)
  - [`CreateWorkOrderInput`](#createworkorderinput)
  - [`CreateZoneShippingMethodInput`](#createzoneshippingmethodinput)
  - [`CreditAccountFilterInput`](#creditaccountfilterinput)
  - [`CreditAccountOutput`](#creditaccountoutput)
  - [`CreditAccountStatus`](#creditaccountstatus)
  - [`CreditAccountStatusInput`](#creditaccountstatusinput)
  - [`CreditCheckOutput`](#creditcheckoutput)
  - [`CreditMemoFilterInput`](#creditmemofilterinput)
  - [`CreditMemoOutput`](#creditmemooutput)
  - [`CreditMemoReason`](#creditmemoreason)
  - [`CreditMemoReasonFilter`](#creditmemoreasonfilter)
  - [`CreditMemoReasonInput`](#creditmemoreasoninput)
  - [`CreditMemoStatus`](#creditmemostatus)
  - [`CreditMemoStatusInput`](#creditmemostatusinput)
  - [`CustomFieldDefinitionInput`](#customfielddefinitioninput)
  - [`CustomFieldDefinitionOutput`](#customfielddefinitionoutput)
  - [`CustomFieldType`](#customfieldtype)
  - [`CustomFieldTypeInput`](#customfieldtypeinput)
  - [`CustomObjectFilterInput`](#customobjectfilterinput)
  - [`CustomObjectOutput`](#customobjectoutput)
  - [`CustomObjectTypeFilterInput`](#customobjecttypefilterinput)
  - [`CustomObjectTypeOutput`](#customobjecttypeoutput)
  - [`CustomerAddressOutput`](#customeraddressoutput)
  - [`CustomerFilterInput`](#customerfilterinput)
  - [`CustomerMetadata`](#customermetadata)
  - [`CustomerMetricsOutput`](#customermetricsoutput)
  - [`CustomerOutput`](#customeroutput)
  - [`CustomerSearchResultOutput`](#customersearchresultoutput)
  - [`CustomerStatus`](#customerstatus)
  - [`CycleCountFilterInput`](#cyclecountfilterinput)
  - [`CycleCountLineOutput`](#cyclecountlineoutput)
  - [`CycleCountOutput`](#cyclecountoutput)
  - [`CycleCountStatus`](#cyclecountstatus)
  - [`DemandForecastOutput`](#demandforecastoutput)
  - [`DemandTrend`](#demandtrend)
  - [`DepreciationEntryOutput`](#depreciationentryoutput)
  - [`DepreciationEntryStatus`](#depreciationentrystatus)
  - [`DepreciationMethod`](#depreciationmethod)
  - [`DepreciationMethodOutput`](#depreciationmethodoutput)
  - [`DepreciationScheduleOutput`](#depreciationscheduleoutput)
  - [`DomainCountOutput`](#domaincountoutput)
  - [`EconomicBudget`](#economicbudget)
  - [`EconomicBudgetStatus`](#economicbudgetstatus)
  - [`EconomicCommitment`](#economiccommitment)
  - [`EconomicMandate`](#economicmandate)
  - [`EdiCountOutput`](#edicountoutput)
  - [`EdiDirection`](#edidirection)
  - [`EdiDocumentFilterInput`](#edidocumentfilterinput)
  - [`EdiDocumentOutput`](#edidocumentoutput)
  - [`EdiStatus`](#edistatus)
  - [`EdiSummaryOutput`](#edisummaryoutput)
  - [`EmbeddingStatsOutput`](#embeddingstatsoutput)
  - [`EnqueuePrintJobInput`](#enqueueprintjobinput)
  - [`EnrollCustomerInput`](#enrollcustomerinput)
  - [`EuVatInfoOutput`](#euvatinfooutput)
  - [`ExchangeRateFilterInput`](#exchangeratefilterinput)
  - [`ExchangeRateOutput`](#exchangerateoutput)
  - [`ExemptionDetailsOutput`](#exemptiondetailsoutput)
  - [`ExportOptionsInput`](#exportoptionsinput)
  - [`ExportReportOutput`](#exportreportoutput)
  - [`FacetConfigInput`](#facetconfiginput)
  - [`FacetConfigOutput`](#facetconfigoutput)
  - [`FacetType`](#facettype)
  - [`FeedbackSummaryOutput`](#feedbacksummaryoutput)
  - [`FieldTransform`](#fieldtransform)
  - [`FixedAssetCategory`](#fixedassetcategory)
  - [`FixedAssetFilterInput`](#fixedassetfilterinput)
  - [`FixedAssetOutput`](#fixedassetoutput)
  - [`FixedAssetStatus`](#fixedassetstatus)
  - [`FraudAssessmentFilterInput`](#fraudassessmentfilterinput)
  - [`FraudAssessmentOutput`](#fraudassessmentoutput)
  - [`FraudDecision`](#frauddecision)
  - [`FraudRuleFilterInput`](#fraudrulefilterinput)
  - [`FraudRuleOutput`](#fraudruleoutput)
  - [`FraudSignalOutput`](#fraudsignaloutput)
  - [`FraudSignalType`](#fraudsignaltype)
  - [`FulfillmentMetricsOutput`](#fulfillmentmetricsoutput)
  - [`FulfillmentStatus`](#fulfillmentstatus)
  - [`FulfillmentType`](#fulfillmenttype)
  - [`GiftCardFilterInput`](#giftcardfilterinput)
  - [`GiftCardOutput`](#giftcardoutput)
  - [`GiftCardStatus`](#giftcardstatus)
  - [`GiftCardTransactionOutput`](#giftcardtransactionoutput)
  - [`GiftCardTransactionType`](#giftcardtransactiontype)
  - [`GlAccountFilterInput`](#glaccountfilterinput)
  - [`GlAccountOutput`](#glaccountoutput)
  - [`GlAccountStatus`](#glaccountstatus)
  - [`GlAccountStatusInput`](#glaccountstatusinput)
  - [`GlAccountType`](#glaccounttype)
  - [`GlAccountTypeFilter`](#glaccounttypefilter)
  - [`GlAccountTypeInput`](#glaccounttypeinput)
  - [`GlBalanceSide`](#glbalanceside)
  - [`GlJournalEntryStatus`](#gljournalentrystatus)
  - [`GlJournalEntryStatusInput`](#gljournalentrystatusinput)
  - [`GlPeriodFilterInput`](#glperiodfilterinput)
  - [`GlPeriodOutput`](#glperiodoutput)
  - [`GlPeriodStatus`](#glperiodstatus)
  - [`HealthGrade`](#healthgrade)
  - [`HybridEncryptionResultOutput`](#hybridencryptionresultoutput)
  - [`HybridPayloadAadParamsInput`](#hybridpayloadaadparamsinput)
  - [`HybridRecipientKeypairOutput`](#hybridrecipientkeypairoutput)
  - [`HybridRecipientPrivateKeyInput`](#hybridrecipientprivatekeyinput)
  - [`HybridRecipientPublicKeyInput`](#hybridrecipientpublickeyinput)
  - [`HybridSignatureBundleOutput`](#hybridsignaturebundleoutput)
  - [`HybridSigningKeypairOutput`](#hybridsigningkeypairoutput)
  - [`ImportConflictPolicy`](#importconflictpolicy)
  - [`ImportOptionsInput`](#importoptionsinput)
  - [`ImportReportOutput`](#importreportoutput)
  - [`InboundShipmentFilterInput`](#inboundshipmentfilterinput)
  - [`InboundShipmentItemOutput`](#inboundshipmentitemoutput)
  - [`InboundShipmentOutput`](#inboundshipmentoutput)
  - [`InboundShipmentStatus`](#inboundshipmentstatus)
  - [`IncomeStatementOutput`](#incomestatementoutput)
  - [`IngestLineItemInput`](#ingestlineiteminput)
  - [`IngestOrderInput`](#ingestorderinput)
  - [`InspectionFilterInput`](#inspectionfilterinput)
  - [`InspectionOutput`](#inspectionoutput)
  - [`InspectionStatus`](#inspectionstatus)
  - [`InspectionType`](#inspectiontype)
  - [`InspectionTypeInput`](#inspectiontypeinput)
  - [`IntegrationFieldMappingFilterInput`](#integrationfieldmappingfilterinput)
  - [`IntegrationFieldMappingOutput`](#integrationfieldmappingoutput)
  - [`IntegrationMappingFilterInput`](#integrationmappingfilterinput)
  - [`IntegrationMappingOutput`](#integrationmappingoutput)
  - [`InventoryHealthOutput`](#inventoryhealthoutput)
  - [`InventoryItemOutput`](#inventoryitemoutput)
  - [`InventoryMovementOutput`](#inventorymovementoutput)
  - [`InventorySearchResultOutput`](#inventorysearchresultoutput)
  - [`InvoiceFilterInput`](#invoicefilterinput)
  - [`InvoiceOutput`](#invoiceoutput)
  - [`InvoiceStatus`](#invoicestatus)
  - [`ItemCostFilterInput`](#itemcostfilterinput)
  - [`ItemCostOutput`](#itemcostoutput)
  - [`JournalEntryFilterInput`](#journalentryfilterinput)
  - [`JournalEntryOutput`](#journalentryoutput)
  - [`JurisdictionFilterInput`](#jurisdictionfilterinput)
  - [`JurisdictionSummaryOutput`](#jurisdictionsummaryoutput)
  - [`KernelA2AEscrowRefPayload`](#kernela2aescrowrefpayload)
  - [`KernelApprovalEvidence`](#kernelapprovalevidence)
  - [`KernelAuthorityEvidence`](#kernelauthorityevidence)
  - [`KernelChargeSubscriptionPayload`](#kernelchargesubscriptionpayload)
  - [`KernelCommand`](#kernelcommand)
  - [`KernelCommandEnvelope`](#kernelcommandenvelope)
  - [`KernelCommandPolicy`](#kernelcommandpolicy)
  - [`KernelCommandType`](#kernelcommandtype)
  - [`KernelCommitCheckoutPayload`](#kernelcommitcheckoutpayload)
  - [`KernelConfirmInventoryReservationPayload`](#kernelconfirminventoryreservationpayload)
  - [`KernelCreateA2AEscrowPayload`](#kernelcreatea2aescrowpayload)
  - [`KernelCreateInventoryItemPayload`](#kernelcreateinventoryitempayload)
  - [`KernelCreatePaymentPayload`](#kernelcreatepaymentpayload)
  - [`KernelCreateProductPayload`](#kernelcreateproductpayload)
  - [`KernelCreateProductVariantPayload`](#kernelcreateproductvariantpayload)
  - [`KernelCreateRefundPayload`](#kernelcreaterefundpayload)
  - [`KernelDisputeA2AEscrowPayload`](#kerneldisputea2aescrowpayload)
  - [`KernelEconomicReceiptContext`](#kerneleconomicreceiptcontext)
  - [`KernelExecutionMode`](#kernelexecutionmode)
  - [`KernelExecutionStatus`](#kernelexecutionstatus)
  - [`KernelFileA2ADisputePayload`](#kernelfilea2adisputepayload)
  - [`KernelOrderPaymentStatus`](#kernelorderpaymentstatus)
  - [`KernelOrderStatus`](#kernelorderstatus)
  - [`KernelPaymentMethodType`](#kernelpaymentmethodtype)
  - [`KernelPolicy`](#kernelpolicy)
  - [`KernelPolicyDecision`](#kernelpolicydecision)
  - [`KernelPostJournalEntryPayload`](#kernelpostjournalentrypayload)
  - [`KernelPrincipal`](#kernelprincipal)
  - [`KernelPrincipalKind`](#kernelprincipalkind)
  - [`KernelProductAttribute`](#kernelproductattribute)
  - [`KernelReceipt`](#kernelreceipt)
  - [`KernelRefundA2AEscrowPayload`](#kernelrefunda2aescrowpayload)
  - [`KernelReleaseInventoryReservationPayload`](#kernelreleaseinventoryreservationpayload)
  - [`KernelReserveInventoryPayload`](#kernelreserveinventorypayload)
  - [`KernelResolveA2ADisputePayload`](#kernelresolvea2adisputepayload)
  - [`KernelRetryDisposition`](#kernelretrydisposition)
  - [`KernelSettleX402IntentPayload`](#kernelsettlex402intentpayload)
  - [`KernelShipOrderPayload`](#kernelshiporderpayload)
  - [`KernelShipmentLine`](#kernelshipmentline)
  - [`KernelStockPolicy`](#kernelstockpolicy)
  - [`KernelSubmitA2ADisputeEvidencePayload`](#kernelsubmita2adisputeevidencepayload)
  - [`KernelTransitionOrderPayload`](#kerneltransitionorderpayload)
  - [`KernelTransitionReturnPayload`](#kerneltransitionreturnpayload)
  - [`KernelVariantOption`](#kernelvariantoption)
  - [`LineItemTaxOutput`](#lineitemtaxoutput)
  - [`LocationOutput`](#locationoutput)
  - [`LotFilterInput`](#lotfilterinput)
  - [`LotOutput`](#lotoutput)
  - [`LotStatus`](#lotstatus)
  - [`LotStatusInput`](#lotstatusinput)
  - [`LowStockItemOutput`](#lowstockitemoutput)
  - [`LoyaltyAccountFilterInput`](#loyaltyaccountfilterinput)
  - [`LoyaltyAccountOutput`](#loyaltyaccountoutput)
  - [`LoyaltyProgramOutput`](#loyaltyprogramoutput)
  - [`LoyaltyProgramStatus`](#loyaltyprogramstatus)
  - [`LoyaltyRewardType`](#loyaltyrewardtype)
  - [`LoyaltyTierInput`](#loyaltytierinput)
  - [`LoyaltyTierOutput`](#loyaltytieroutput)
  - [`LoyaltyTransactionOutput`](#loyaltytransactionoutput)
  - [`LoyaltyTransactionType`](#loyaltytransactiontype)
  - [`MapPurgatoryLineInput`](#mappurgatorylineinput)
  - [`MappingLookupInput`](#mappinglookupinput)
  - [`MoneyWire`](#moneywire)
  - [`NcrFilterInput`](#ncrfilterinput)
  - [`NcrOutput`](#ncroutput)
  - [`NcrSeverity`](#ncrseverity)
  - [`NcrSeverityInput`](#ncrseverityinput)
  - [`NcrSource`](#ncrsource)
  - [`NcrSourceInput`](#ncrsourceinput)
  - [`NcrStatus`](#ncrstatus)
  - [`OpenOptions`](#openoptions)
  - [`OrderAddressInput`](#orderaddressinput)
  - [`OrderAddressOutput`](#orderaddressoutput)
  - [`OrderFilterInput`](#orderfilterinput)
  - [`OrderItemOutput`](#orderitemoutput)
  - [`OrderOutput`](#orderoutput)
  - [`OrderSearchResultOutput`](#ordersearchresultoutput)
  - [`OrderStatus`](#orderstatus)
  - [`OrderStatusBreakdownOutput`](#orderstatusbreakdownoutput)
  - [`OrderStatusUpdate`](#orderstatusupdate)
  - [`PairStationResultOutput`](#pairstationresultoutput)
  - [`PauseSubscriptionInput`](#pausesubscriptioninput)
  - [`PaymentFilterInput`](#paymentfilterinput)
  - [`PaymentMethodType`](#paymentmethodtype)
  - [`PaymentObligationDashboardOutput`](#paymentobligationdashboardoutput)
  - [`PaymentObligationFilterInput`](#paymentobligationfilterinput)
  - [`PaymentObligationOutput`](#paymentobligationoutput)
  - [`PaymentObligationStatus`](#paymentobligationstatus)
  - [`PaymentOutput`](#paymentoutput)
  - [`PaymentStatus`](#paymentstatus)
  - [`PaymentTransactionStatus`](#paymenttransactionstatus)
  - [`PerformanceObligationOutput`](#performanceobligationoutput)
  - [`PickTaskFilterInput`](#picktaskfilterinput)
  - [`PickTaskOutput`](#picktaskoutput)
  - [`PickTaskStatus`](#picktaskstatus)
  - [`PickTaskStatusInput`](#picktaskstatusinput)
  - [`PortableDomainsOutput`](#portabledomainsoutput)
  - [`PrepaymentApplicationOutput`](#prepaymentapplicationoutput)
  - [`PrepaymentFilterInput`](#prepaymentfilterinput)
  - [`PrepaymentOutput`](#prepaymentoutput)
  - [`PrepaymentStatus`](#prepaymentstatus)
  - [`PrepaymentTargetType`](#prepaymenttargettype)
  - [`PriceAdjustmentType`](#priceadjustmenttype)
  - [`PriceLevelEntryOutput`](#pricelevelentryoutput)
  - [`PriceLevelFilterInput`](#pricelevelfilterinput)
  - [`PriceLevelOutput`](#priceleveloutput)
  - [`PriceScheduleEntryOutput`](#pricescheduleentryoutput)
  - [`PriceScheduleFilterInput`](#priceschedulefilterinput)
  - [`PriceScheduleOutput`](#pricescheduleoutput)
  - [`PrintJobFilterInput`](#printjobfilterinput)
  - [`PrintJobOutput`](#printjoboutput)
  - [`PrintJobStatus`](#printjobstatus)
  - [`PrintPayloadKind`](#printpayloadkind)
  - [`PrintStationFilterInput`](#printstationfilterinput)
  - [`PrintStationOutput`](#printstationoutput)
  - [`ProductFilterInput`](#productfilterinput)
  - [`ProductOutput`](#productoutput)
  - [`ProductPerformanceOutput`](#productperformanceoutput)
  - [`ProductSearchResultOutput`](#productsearchresultoutput)
  - [`ProductStatus`](#productstatus)
  - [`ProductTaxCategory`](#producttaxcategory)
  - [`ProductVariantOutput`](#productvariantoutput)
  - [`ProductionBatchFilterInput`](#productionbatchfilterinput)
  - [`ProductionBatchOutput`](#productionbatchoutput)
  - [`ProductionBatchStatus`](#productionbatchstatus)
  - [`PromotionFilterInput`](#promotionfilterinput)
  - [`PromotionLineItemInput`](#promotionlineiteminput)
  - [`PromotionOutput`](#promotionoutput)
  - [`PromotionStacking`](#promotionstacking)
  - [`PromotionStackingInput`](#promotionstackinginput)
  - [`PromotionStatus`](#promotionstatus)
  - [`PromotionTarget`](#promotiontarget)
  - [`PromotionTargetInput`](#promotiontargetinput)
  - [`PromotionTrigger`](#promotiontrigger)
  - [`PromotionTriggerInput`](#promotiontriggerinput)
  - [`PromotionType`](#promotiontype)
  - [`PromotionTypeInput`](#promotiontypeinput)
  - [`PromotionUsageOutput`](#promotionusageoutput)
  - [`PurchaseOrderFilterInput`](#purchaseorderfilterinput)
  - [`PurchaseOrderOutput`](#purchaseorderoutput)
  - [`PurchaseOrderStatus`](#purchaseorderstatus)
  - [`PurgatoryFilterInput`](#purgatoryfilterinput)
  - [`PurgatoryLineItemOutput`](#purgatorylineitemoutput)
  - [`PurgatoryOrderOutput`](#purgatoryorderoutput)
  - [`QualityHoldFilterInput`](#qualityholdfilterinput)
  - [`QualityHoldOutput`](#qualityholdoutput)
  - [`QualityHoldStatus`](#qualityholdstatus)
  - [`QualityHoldType`](#qualityholdtype)
  - [`QualityHoldTypeFilter`](#qualityholdtypefilter)
  - [`QualityHoldTypeInput`](#qualityholdtypeinput)
  - [`ReceiptFilterInput`](#receiptfilterinput)
  - [`ReceiptOutput`](#receiptoutput)
  - [`ReceiptStatus`](#receiptstatus)
  - [`ReceiptStatusInput`](#receiptstatusinput)
  - [`ReceiptType`](#receipttype)
  - [`ReceiptTypeFilter`](#receipttypefilter)
  - [`ReceiptTypeInput`](#receipttypeinput)
  - [`RecognitionMethod`](#recognitionmethod)
  - [`RecognitionMethodOutput`](#recognitionmethodoutput)
  - [`RecordActivityInput`](#recordactivityinput)
  - [`RecordCycleCountLineInput`](#recordcyclecountlineinput)
  - [`RecordPaymentInput`](#recordpaymentinput)
  - [`RefundOutput`](#refundoutput)
  - [`RefundStatus`](#refundstatus)
  - [`ReservationOutput`](#reservationoutput)
  - [`ReservationStatus`](#reservationstatus)
  - [`RestoreOptionsInput`](#restoreoptionsinput)
  - [`RestoreReportOutput`](#restorereportoutput)
  - [`ReturnFilterInput`](#returnfilterinput)
  - [`ReturnMetricsOutput`](#returnmetricsoutput)
  - [`ReturnOutput`](#returnoutput)
  - [`ReturnReason`](#returnreason)
  - [`ReturnStatus`](#returnstatus)
  - [`RevaluationLineOutput`](#revaluationlineoutput)
  - [`RevaluationOutput`](#revaluationoutput)
  - [`RevenueByPeriodOutput`](#revenuebyperiodoutput)
  - [`RevenueContractFilterInput`](#revenuecontractfilterinput)
  - [`RevenueContractOutput`](#revenuecontractoutput)
  - [`RevenueContractStatus`](#revenuecontractstatus)
  - [`RevenueEntryStatus`](#revenueentrystatus)
  - [`RevenueForecastOutput`](#revenueforecastoutput)
  - [`RevenueScheduleEntryOutput`](#revenuescheduleentryoutput)
  - [`RevenueScheduleOutput`](#revenuescheduleoutput)
  - [`ReviewFilterInput`](#reviewfilterinput)
  - [`ReviewOutput`](#reviewoutput)
  - [`ReviewStatus`](#reviewstatus)
  - [`ReviewSummaryOutput`](#reviewsummaryoutput)
  - [`RewardFilterInput`](#rewardfilterinput)
  - [`RewardOutput`](#rewardoutput)
  - [`RoundingMode`](#roundingmode)
  - [`SalesSummaryOutput`](#salessummaryoutput)
  - [`SearchConfigFilterInput`](#searchconfigfilterinput)
  - [`SearchConfigOutput`](#searchconfigoutput)
  - [`SearchFieldInput`](#searchfieldinput)
  - [`SearchFieldOutput`](#searchfieldoutput)
  - [`SearchTokenizer`](#searchtokenizer)
  - [`SegmentFilterInput`](#segmentfilterinput)
  - [`SegmentMembershipOutput`](#segmentmembershipoutput)
  - [`SegmentOperator`](#segmentoperator)
  - [`SegmentOutput`](#segmentoutput)
  - [`SegmentRuleInput`](#segmentruleinput)
  - [`SegmentRuleOutput`](#segmentruleoutput)
  - [`SegmentType`](#segmenttype)
  - [`SerialFilterInput`](#serialfilterinput)
  - [`SerialOutput`](#serialoutput)
  - [`SerialStatus`](#serialstatus)
  - [`SerialStatusInput`](#serialstatusinput)
  - [`SetCartPaymentInput`](#setcartpaymentinput)
  - [`SetCartShippingInput`](#setcartshippinginput)
  - [`SetExchangeRateInput`](#setexchangerateinput)
  - [`SetItemCostInput`](#setitemcostinput)
  - [`ShipmentFilterInput`](#shipmentfilterinput)
  - [`ShipmentMethod`](#shipmentmethod)
  - [`ShipmentMethodInput`](#shipmentmethodinput)
  - [`ShipmentOutput`](#shipmentoutput)
  - [`ShipmentStatus`](#shipmentstatus)
  - [`ShippingCarrier`](#shippingcarrier)
  - [`ShippingCarrierFilter`](#shippingcarrierfilter)
  - [`ShippingCarrierInput`](#shippingcarrierinput)
  - [`ShippingConditionInput`](#shippingconditioninput)
  - [`ShippingConditionOutput`](#shippingconditionoutput)
  - [`ShippingMethodType`](#shippingmethodtype)
  - [`ShippingRateOutput`](#shippingrateoutput)
  - [`ShippingZoneFilterInput`](#shippingzonefilterinput)
  - [`ShippingZoneOutput`](#shippingzoneoutput)
  - [`SkipBillingCycleInput`](#skipbillingcycleinput)
  - [`StockLevelOutput`](#stockleveloutput)
  - [`StockPolicy`](#stockpolicy)
  - [`StockSnapshotFilterInput`](#stocksnapshotfilterinput)
  - [`StockSnapshotLineOutput`](#stocksnapshotlineoutput)
  - [`StockSnapshotOutput`](#stocksnapshotoutput)
  - [`StoreCreditFilterInput`](#storecreditfilterinput)
  - [`StoreCreditOutput`](#storecreditoutput)
  - [`StoreCreditReason`](#storecreditreason)
  - [`StoreCreditStatus`](#storecreditstatus)
  - [`StoreCreditTransactionOutput`](#storecredittransactionoutput)
  - [`StoreCreditTransactionType`](#storecredittransactiontype)
  - [`StoreCurrencySettingsInput`](#storecurrencysettingsinput)
  - [`StoreCurrencySettingsOutput`](#storecurrencysettingsoutput)
  - [`StrictEncryptionResultOutput`](#strictencryptionresultoutput)
  - [`StrictRecipientKeypairOutput`](#strictrecipientkeypairoutput)
  - [`StrictRecipientPrivateKeyInput`](#strictrecipientprivatekeyinput)
  - [`StrictRecipientPublicKeyInput`](#strictrecipientpublickeyinput)
  - [`StrictSigningKeypairOutput`](#strictsigningkeypairoutput)
  - [`SubscriptionBillingInterval`](#subscriptionbillinginterval)
  - [`SubscriptionBillingIntervalInput`](#subscriptionbillingintervalinput)
  - [`SubscriptionEventOutput`](#subscriptioneventoutput)
  - [`SubscriptionEventType`](#subscriptioneventtype)
  - [`SubscriptionFilterInput`](#subscriptionfilterinput)
  - [`SubscriptionOutput`](#subscriptionoutput)
  - [`SubscriptionPlanFilterInput`](#subscriptionplanfilterinput)
  - [`SubscriptionPlanOutput`](#subscriptionplanoutput)
  - [`SubscriptionPlanStatus`](#subscriptionplanstatus)
  - [`SubscriptionStatus`](#subscriptionstatus)
  - [`SupplierFilterInput`](#supplierfilterinput)
  - [`SupplierOutput`](#supplieroutput)
  - [`SupplierSkuFilterInput`](#supplierskufilterinput)
  - [`SupplierSkuOutput`](#supplierskuoutput)
  - [`SynonymGroupInput`](#synonymgroupinput)
  - [`SynonymGroupOutput`](#synonymgroupoutput)
  - [`TaxAddressInput`](#taxaddressinput)
  - [`TaxBreakdownOutput`](#taxbreakdownoutput)
  - [`TaxCalculationInput`](#taxcalculationinput)
  - [`TaxCalculationMethod`](#taxcalculationmethod)
  - [`TaxCalculationOutput`](#taxcalculationoutput)
  - [`TaxCompoundMethod`](#taxcompoundmethod)
  - [`TaxDetailOutput`](#taxdetailoutput)
  - [`TaxExemptionOutput`](#taxexemptionoutput)
  - [`TaxExemptionType`](#taxexemptiontype)
  - [`TaxExemptionTypeInput`](#taxexemptiontypeinput)
  - [`TaxJurisdictionLevel`](#taxjurisdictionlevel)
  - [`TaxJurisdictionOutput`](#taxjurisdictionoutput)
  - [`TaxLineItemInput`](#taxlineiteminput)
  - [`TaxRateFilterInput`](#taxratefilterinput)
  - [`TaxRateOutput`](#taxrateoutput)
  - [`TaxSettingsInput`](#taxsettingsinput)
  - [`TaxSettingsOutput`](#taxsettingsoutput)
  - [`TaxType`](#taxtype)
  - [`ThreeWayMatchLineOutput`](#threewaymatchlineoutput)
  - [`ThreeWayMatchOutput`](#threewaymatchoutput)
  - [`ThreeWayMatchStatus`](#threewaymatchstatus)
  - [`TopCustomerOutput`](#topcustomeroutput)
  - [`TopProductOutput`](#topproductoutput)
  - [`TopologySnapshotFilterInput`](#topologysnapshotfilterinput)
  - [`TopologySnapshotOutput`](#topologysnapshotoutput)
  - [`TransferOrderFilterInput`](#transferorderfilterinput)
  - [`TransferOrderItemOutput`](#transferorderitemoutput)
  - [`TransferOrderOutput`](#transferorderoutput)
  - [`TransferOrderStatus`](#transferorderstatus)
  - [`TrialBalanceOutput`](#trialbalanceoutput)
  - [`UnitClassFilterInput`](#unitclassfilterinput)
  - [`UnitClassOutput`](#unitclassoutput)
  - [`UnitConversionRuleFilterInput`](#unitconversionrulefilterinput)
  - [`UnitConversionRuleOutput`](#unitconversionruleoutput)
  - [`UnitOfMeasureFilterInput`](#unitofmeasurefilterinput)
  - [`UnitOfMeasureOutput`](#unitofmeasureoutput)
  - [`UpdateAgentIdentityInput`](#updateagentidentityinput)
  - [`UpdateCartInput`](#updatecartinput)
  - [`UpdateCartItemInput`](#updatecartiteminput)
  - [`UpdateChannelInput`](#updatechannelinput)
  - [`UpdateCompanyInput`](#updatecompanyinput)
  - [`UpdateCustomObjectInput`](#updatecustomobjectinput)
  - [`UpdateCustomObjectTypeInput`](#updatecustomobjecttypeinput)
  - [`UpdateCustomerInput`](#updatecustomerinput)
  - [`UpdateFixedAssetInput`](#updatefixedassetinput)
  - [`UpdateFraudRuleInput`](#updatefraudruleinput)
  - [`UpdateGiftCardInput`](#updategiftcardinput)
  - [`UpdateIntegrationFieldMappingInput`](#updateintegrationfieldmappinginput)
  - [`UpdateIntegrationMappingInput`](#updateintegrationmappinginput)
  - [`UpdatePriceLevelInput`](#updatepricelevelinput)
  - [`UpdatePriceScheduleInput`](#updatepricescheduleinput)
  - [`UpdateProductInput`](#updateproductinput)
  - [`UpdateProductionBatchInput`](#updateproductionbatchinput)
  - [`UpdatePromotionInput`](#updatepromotioninput)
  - [`UpdateRevenueContractInput`](#updaterevenuecontractinput)
  - [`UpdateReviewInput`](#updatereviewinput)
  - [`UpdateSearchConfigInput`](#updatesearchconfiginput)
  - [`UpdateSegmentInput`](#updatesegmentinput)
  - [`UpdateShippingZoneInput`](#updateshippingzoneinput)
  - [`UpdateSubscriptionInput`](#updatesubscriptioninput)
  - [`UpdateSubscriptionPlanInput`](#updatesubscriptionplaninput)
  - [`UpdateSupplierSkuInput`](#updatesupplierskuinput)
  - [`UpdateWishlistInput`](#updatewishlistinput)
  - [`UsStateTaxInfoOutput`](#usstatetaxinfooutput)
  - [`ValidationSummaryOutput`](#validationsummaryoutput)
  - [`VectorEntityType`](#vectorentitytype)
  - [`VectorSearchResultOutput`](#vectorsearchresultoutput)
  - [`VendorCreditApplicationOutput`](#vendorcreditapplicationoutput)
  - [`VendorCreditFilterInput`](#vendorcreditfilterinput)
  - [`VendorCreditOutput`](#vendorcreditoutput)
  - [`VendorCreditStatus`](#vendorcreditstatus)
  - [`VendorCreditTargetType`](#vendorcredittargettype)
  - [`VendorReturnFilterInput`](#vendorreturnfilterinput)
  - [`VendorReturnItemOutput`](#vendorreturnitemoutput)
  - [`VendorReturnOutput`](#vendorreturnoutput)
  - [`VendorReturnReason`](#vendorreturnreason)
  - [`VendorReturnStatus`](#vendorreturnstatus)
  - [`VesHashDomain`](#veshashdomain)
  - [`WarehouseFilterInput`](#warehousefilterinput)
  - [`WarehouseLocationType`](#warehouselocationtype)
  - [`WarehouseLocationTypeInput`](#warehouselocationtypeinput)
  - [`WarehouseOutput`](#warehouseoutput)
  - [`WarehouseType`](#warehousetype)
  - [`WarehouseTypeFilter`](#warehousetypefilter)
  - [`WarehouseTypeInput`](#warehousetypeinput)
  - [`WarrantyClaimOutput`](#warrantyclaimoutput)
  - [`WarrantyClaimResolution`](#warrantyclaimresolution)
  - [`WarrantyClaimResolutionInput`](#warrantyclaimresolutioninput)
  - [`WarrantyClaimStatus`](#warrantyclaimstatus)
  - [`WarrantyFilterInput`](#warrantyfilterinput)
  - [`WarrantyOutput`](#warrantyoutput)
  - [`WarrantyStatus`](#warrantystatus)
  - [`WarrantyType`](#warrantytype)
  - [`WarrantyTypeInput`](#warrantytypeinput)
  - [`WaveFilterInput`](#wavefilterinput)
  - [`WaveOutput`](#waveoutput)
  - [`WaveStatus`](#wavestatus)
  - [`WaveStatusInput`](#wavestatusinput)
  - [`WebhookOutput`](#webhookoutput)
  - [`WishlistFilterInput`](#wishlistfilterinput)
  - [`WishlistItemOutput`](#wishlistitemoutput)
  - [`WishlistOutput`](#wishlistoutput)
  - [`WorkOrderFilterInput`](#workorderfilterinput)
  - [`WorkOrderOutput`](#workorderoutput)
  - [`WorkOrderPriority`](#workorderpriority)
  - [`WorkOrderStatus`](#workorderstatus)
  - [`X402A2ASkill`](#x402a2askill)
  - [`X402A2ASkillInput`](#x402a2askillinput)
  - [`X402AgentCardFilterInput`](#x402agentcardfilterinput)
  - [`X402AgentCardInput`](#x402agentcardinput)
  - [`X402AgentCardOutput`](#x402agentcardoutput)
  - [`X402Asset`](#x402asset)
  - [`X402AssetInput`](#x402assetinput)
  - [`X402CreateIntentInput`](#x402createintentinput)
  - [`X402CreditAccountOutput`](#x402creditaccountoutput)
  - [`X402CreditAdjustmentInput`](#x402creditadjustmentinput)
  - [`X402CreditBalanceInput`](#x402creditbalanceinput)
  - [`X402CreditDirection`](#x402creditdirection)
  - [`X402CreditDirectionInput`](#x402creditdirectioninput)
  - [`X402CreditTransactionFilterInput`](#x402credittransactionfilterinput)
  - [`X402CreditTransactionOutput`](#x402credittransactionoutput)
  - [`X402IntentFilterInput`](#x402intentfilterinput)
  - [`X402IntentOutput`](#x402intentoutput)
  - [`X402IntentStatus`](#x402intentstatus)
  - [`X402IntentStatusInput`](#x402intentstatusinput)
  - [`X402Network`](#x402network)
  - [`X402NetworkInput`](#x402networkinput)
  - [`X402PublicKeyBundleInput`](#x402publickeybundleinput)
  - [`X402PublicKeyBundleOutput`](#x402publickeybundleoutput)
  - [`X402SignIntentInput`](#x402signintentinput)
  - [`X402SignatureBundleInput`](#x402signaturebundleinput)
  - [`X402SignatureBundleOutput`](#x402signaturebundleoutput)
  - [`X402SignatureScheme`](#x402signaturescheme)
  - [`X402SigningHashInput`](#x402signinghashinput)
  - [`X402TrustLevel`](#x402trustlevel)
  - [`X402TrustLevelInput`](#x402trustlevelinput)
  - [`ZoneShippingMethodFilterInput`](#zoneshippingmethodfilterinput)
  - [`ZoneShippingMethodOutput`](#zoneshippingmethodoutput)
  - [`ZoneShippingRateOutput`](#zoneshippingrateoutput)
  - [`ZoneShippingRateRequestInput`](#zoneshippingraterequestinput)

## Commerce

JavaScript-friendly Commerce instance

- **`new Commerce(dbPath: string)`**

  Create a new Commerce instance with a database path
  Use ":memory:" for an in-memory database

- **`get isClosed: boolean`**

  Whether `close` has run.

- **`kernelFeatures(): Array<string>`**

  Execution features implemented by this native binary. Hosts must check
  this before relying on optional safety fields that old binaries ignore.

- **`checkoutSnapshot(cartId: string): Promise<CheckoutSnapshot>`**

  Read exact quote terms and their fingerprint from the same cart snapshot.
  Keep this result with the issued quote; never recalculate it at acceptance.

  The declared shape (`CheckoutSnapshot`, `scripts/types/kernel.d.ts`)
  mirrors `stateset_core::Cart` as serde writes it — snake_case, exact
  decimal strings — not the camelCase `CartOutput` of the `carts` API.

  Types: [`CheckoutSnapshot`](#checkoutsnapshot)

- **`static open(dbPath: string, options?: OpenOptions | undefined | null): Promise<Commerce>`**

  Open a database off the event loop.

  The constructor runs every pending migration synchronously on the
  JavaScript thread; this does the same work on a worker and resolves to
  the ready instance.

  Types: [`OpenOptions`](#openoptions)

- **`close(): Promise<void>`**

  Release the engine. Calls already in flight complete; every later call
  rejects with `PRECONDITION_FAILED`. Idempotent.

- **`executeKernelCommand(command: KernelCommand, policy: KernelPolicy): Promise<KernelReceipt>`**

  Execute a versioned commerce kernel command under host-supplied policy.

  `policy` must come from trusted application configuration. It must not
  be copied from model-generated tool arguments.

  `KernelCommand`, `KernelPolicy` and `KernelReceipt` are declared in
  `scripts/types/kernel.d.ts` and mirror `stateset_core::kernel`'s serde
  contracts; the JSON passes through unchanged and serde validates it.

  Types: [`KernelCommand`](#kernelcommand), [`KernelPolicy`](#kernelpolicy), [`KernelReceipt`](#kernelreceipt)

- **`provisionEconomicBudget(budget: EconomicBudget): Promise<EconomicBudgetStatus>`**

  Provision immutable, durable monetary authority for governed commands.
  This is an operator API and should not be exposed as a model tool.

  Types: [`EconomicBudget`](#economicbudget), [`EconomicBudgetStatus`](#economicbudgetstatus)

- **`economicBudgetStatus(budgetId: string): Promise<EconomicBudgetStatus | null>`**

  Read exact committed and available balances for a durable budget.
  Resolves to `null` when no budget carries `budget_id`.

  Types: [`EconomicBudgetStatus`](#economicbudgetstatus)

- **`vector(apiKey: string): VectorSearch`**

  Create a vector search instance with the given OpenAI API key

  Vector search enables semantic similarity search across products,
  customers, orders, and inventory items using OpenAI embeddings.

  Types: [`VectorSearch`](#commercevectorapikey)

### Sub-API accessors

| Accessor | Class | Description |
|---|---|---|
| [`commerce.customers`](#commercecustomers) | `Customers` | Get the customers API |
| [`commerce.orders`](#commerceorders) | `Orders` | Get the orders API |
| [`commerce.products`](#commerceproducts) | `Products` | Get the products API |
| [`commerce.customObjects`](#commercecustomobjects) | `CustomObjects` | Get the custom objects API (custom states / metaobjects) |
| [`commerce.customStates`](#commercecustomobjects) | `CustomObjects` | Alias for `custom_objects` (for users who prefer the "custom states" name) |
| [`commerce.inventory`](#commerceinventory) | `Inventory` | Get the inventory API |
| [`commerce.returns`](#commercereturns) | `Returns` | Get the returns API |
| [`commerce.giftCards`](#commercegiftcards) | `GiftCards` | Get the gift cards API |
| [`commerce.loyalty`](#commerceloyalty) | `Loyalty` | Get the loyalty API |
| [`commerce.storeCredits`](#commercestorecredits) | `StoreCredits` | Get the store credits API |
| [`commerce.reviews`](#commercereviews) | `Reviews` | Get the product reviews API |
| [`commerce.wishlists`](#commercewishlists) | `Wishlists` | Get the wishlists API |
| [`commerce.segments`](#commercesegments) | `Segments` | Get the customer segments API |
| [`commerce.payments`](#commercepayments) | `Payments` | Get the payments API |
| [`commerce.x402`](#commercex402) | `X402` | Get the x402 payment protocol API |
| [`commerce.shipments`](#commerceshipments) | `Shipments` | Get the shipments API |
| [`commerce.warranties`](#commercewarranties) | `Warranties` | Get the warranties API |
| [`commerce.purchaseOrders`](#commercepurchaseorders) | `PurchaseOrders` | Get the purchase orders API |
| [`commerce.invoices`](#commerceinvoices) | `Invoices` | Get the invoices API |
| [`commerce.bom`](#commercebom) | `Bom` | Get the bill of materials API |
| [`commerce.workOrders`](#commerceworkorders) | `WorkOrders` | Get the work orders API |
| [`commerce.carts`](#commercecarts) | `Carts` | Get the carts/checkout API |
| [`commerce.analytics`](#commerceanalytics) | `Analytics` | Get the analytics API |
| [`commerce.currency`](#commercecurrency) | `CurrencyOperations` | Get the currency API |
| [`commerce.subscriptions`](#commercesubscriptions) | `Subscriptions` | Get the subscriptions API |
| [`commerce.promotions`](#commercepromotions) | `Promotions` | Get the promotions API |
| [`commerce.tax`](#commercetax) | `Tax` | Get the tax API |
| [`commerce.quality`](#commercequality) | `Quality` | Get the quality control API |
| [`commerce.lots`](#commercelots) | `Lots` | Get the lot/batch tracking API |
| [`commerce.serials`](#commerceserials) | `Serials` | Get the serial number API |
| [`commerce.warehouse`](#commercewarehouse) | `Warehouse` | Get the warehouse API |
| [`commerce.receiving`](#commercereceiving) | `Receiving` | Get the receiving API |
| [`commerce.fulfillment`](#commercefulfillment) | `Fulfillment` | Get the fulfillment API |
| [`commerce.accountsPayable`](#commerceaccountspayable) | `AccountsPayable` | Get the accounts payable API |
| [`commerce.accountsReceivable`](#commerceaccountsreceivable) | `AccountsReceivable` | Get the accounts receivable API |
| [`commerce.costAccounting`](#commercecostaccounting) | `CostAccounting` | Get the cost accounting API |
| [`commerce.credit`](#commercecredit) | `Credit` | Get the credit management API |
| [`commerce.backorder`](#commercebackorder) | `Backorders` | Get the backorder management API |
| [`commerce.backorders`](#commercebackorder) | `Backorders` | Alias for `backorder`, matching the class name and the other plural sub-API names (`orders`, `returns`, `shipments`). |
| [`commerce.generalLedger`](#commercegeneralledger) | `GeneralLedger` | Get the general ledger API |
| [`commerce.fixedAssets`](#commercefixedassets) | `FixedAssets` | Get the fixed assets API |
| [`commerce.revenueRecognition`](#commercerevenuerecognition) | `RevenueRecognition` | Get the revenue recognition (ASC 606) API |
| [`commerce.cycleCounts`](#commercecyclecounts) | `CycleCounts` | Get the cycle counts API |
| [`commerce.ediDocuments`](#commerceedidocuments) | `EdiDocuments` | Get the EDI documents API (trading-partner document tracking) |
| [`commerce.activityLogs`](#commerceactivitylogs) | `ActivityLogs` | Get the activity logs API (append-only subject history) |
| [`commerce.channels`](#commercechannels) | `Channels` | Get the channels API (sales / fulfillment channels) |
| [`commerce.companies`](#commercecompanies) | `Companies` | Get the companies API (B2B accounts and contacts) |
| [`commerce.unitsOfMeasure`](#commerceunitsofmeasure) | `UnitsOfMeasure` | Get the units of measure API (unit classes, UOMs, conversion rules) |
| [`commerce.shippingZones`](#commerceshippingzones) | `ShippingZones` | Get the shipping zones API (geographic zones, methods, rates) |
| [`commerce.stockSnapshots`](#commercestocksnapshots) | `StockSnapshots` | Get the stock snapshots API (point-in-time inventory) |
| [`commerce.printStations`](#commerceprintstations) | `PrintStations` | Get the print stations API (paired agents + print job queue) |
| [`commerce.integrationMappings`](#commerceintegrationmappings) | `IntegrationMappings` | Get the integration mappings API (external↔internal value translation) |
| [`commerce.integrationFieldMappings`](#commerceintegrationfieldmappings) | `IntegrationFieldMappings` | Get the integration field mappings API (field-path mappings) |
| [`commerce.paymentObligations`](#commercepaymentobligations) | `PaymentObligations` | Get the payment obligations API (scheduled AP payments) |
| [`commerce.maintenance`](#commercemaintenance) | `Maintenance` | Get the maintenance API (backup, restore, export, import) |
| [`commerce.purgatory`](#commercepurgatory) | `Purgatory` | Get the purgatory API (order ingestion staging) |
| [`commerce.topologySnapshots`](#commercetopologysnapshots) | `TopologySnapshots` | Get the topology snapshots API (operational topology health) |
| [`commerce.fraud`](#commercefraud) | `Fraud` | Get the fraud API (risk assessments and detection rules) |
| [`commerce.searchConfig`](#commercesearchconfig) | `SearchConfigs` | Get the search configuration API (search tuning profiles) |
| [`commerce.erc8004`](#commerceerc8004) | `Erc8004` | Get the ERC-8004 API (trustless agent identity, reputation, validation) |
| [`commerce.vendorReturns`](#commercevendorreturns) | `VendorReturns` | Get the vendor returns API (return-to-supplier) |
| [`commerce.prepayments`](#commerceprepayments) | `Prepayments` | Get the prepayments API (advance payments to suppliers) |
| [`commerce.vendorCredits`](#commercevendorcredits) | `VendorCredits` | Get the vendor credits API (supplier-owed credits) |
| [`commerce.priceSchedules`](#commercepriceschedules) | `PriceSchedules` | Get the price schedules API (time-bounded pricing) |
| [`commerce.priceLevels`](#commercepricelevels) | `PriceLevels` | Get the price levels API (B2B pricing tiers) |
| [`commerce.transferOrders`](#commercetransferorders) | `TransferOrders` | Get the transfer orders API (inter-warehouse stock movement) |
| [`commerce.productionBatches`](#commerceproductionbatches) | `ProductionBatches` | Get the production batches API (grouping manufacturing work orders) |
| [`commerce.supplierSkus`](#commercesupplierskus) | `SupplierSkus` | Get the supplier SKUs API (per-supplier SKU / unit-cost overrides) |
| [`commerce.inboundShipments`](#commerceinboundshipments) | `InboundShipments` | Get the inbound shipments API (advance ship notices) |
| [`commerce.events`](#commerceevents) | `Events` | Get the events API (pub/sub and webhook management) |

## Sub-APIs

Each sub-API is a property of a `Commerce` instance; the heading is the access path.

### commerce.customers

Class `Customers`.

- **`create(input: CreateCustomerInput): Promise<CustomerOutput>`**

  Types: [`CreateCustomerInput`](#createcustomerinput), [`CustomerOutput`](#customeroutput)

- **`get(id: string): Promise<CustomerOutput | null>`**

  Types: [`CustomerOutput`](#customeroutput)

- **`getByEmail(email: string): Promise<CustomerOutput | null>`**

  Types: [`CustomerOutput`](#customeroutput)

- **`list(filter?: CustomerFilterInput | undefined | null): Promise<Array<CustomerOutput>>`**

  List customers, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every customer).

  Types: [`CustomerFilterInput`](#customerfilterinput), [`CustomerOutput`](#customeroutput)

- **`count(): Promise<number>`**

- **`update(id: string, input: UpdateCustomerInput): Promise<CustomerOutput>`**

  Types: [`UpdateCustomerInput`](#updatecustomerinput), [`CustomerOutput`](#customeroutput)

- **`delete(id: string): Promise<void>`**

- **`findOrCreate(input: CreateCustomerInput): Promise<CustomerOutput>`**

  Types: [`CreateCustomerInput`](#createcustomerinput), [`CustomerOutput`](#customeroutput)

- **`addAddress(input: CreateCustomerAddressInput): Promise<CustomerAddressOutput>`**

  Types: [`CreateCustomerAddressInput`](#createcustomeraddressinput), [`CustomerAddressOutput`](#customeraddressoutput)

- **`getAddresses(customerId: string): Promise<Array<CustomerAddressOutput>>`**

  Types: [`CustomerAddressOutput`](#customeraddressoutput)

- **`updateAddress(addressId: string, input: CreateCustomerAddressInput): Promise<CustomerAddressOutput>`**

  Types: [`CreateCustomerAddressInput`](#createcustomeraddressinput), [`CustomerAddressOutput`](#customeraddressoutput)

- **`deleteAddress(addressId: string): Promise<void>`**

- **`setDefaultAddress(customerId: string, addressId: string, addressType: AddressType): Promise<void>`**

  Types: [`AddressType`](#addresstype)

### commerce.orders

Class `Orders`.

- **`create(input: CreateOrderInput): Promise<OrderOutput>`**

  Types: [`CreateOrderInput`](#createorderinput), [`OrderOutput`](#orderoutput)

- **`createExact(input: CreateOrderExactInput): Promise<OrderOutput>`**

  Create an order without any floating-point conversion.

  Types: [`CreateOrderExactInput`](#createorderexactinput), [`OrderOutput`](#orderoutput)

- **`get(id: string): Promise<OrderOutput | null>`**

  Types: [`OrderOutput`](#orderoutput)

- **`list(filter?: OrderFilterInput | undefined | null): Promise<Array<OrderOutput>>`**

  List orders, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every order).

  Types: [`OrderFilterInput`](#orderfilterinput), [`OrderOutput`](#orderoutput)

- **`updateStatus(id: string, status: OrderStatusUpdate): Promise<OrderOutput>`**

  Types: [`OrderStatusUpdate`](#orderstatusupdate), [`OrderOutput`](#orderoutput)

- **`ship(id: string, trackingNumber?: string | undefined | null): Promise<OrderOutput>`**

  Types: [`OrderOutput`](#orderoutput)

- **`cancel(id: string): Promise<OrderOutput>`**

  Types: [`OrderOutput`](#orderoutput)

- **`count(): Promise<number>`**

### commerce.products

Class `Products`.

- **`create(input: CreateProductInput): Promise<ProductOutput>`**

  Types: [`CreateProductInput`](#createproductinput), [`ProductOutput`](#productoutput)

- **`get(id: string): Promise<ProductOutput | null>`**

  Types: [`ProductOutput`](#productoutput)

- **`getVariantBySku(sku: string): Promise<ProductVariantOutput | null>`**

  Types: [`ProductVariantOutput`](#productvariantoutput)

- **`list(filter?: ProductFilterInput | undefined | null): Promise<Array<ProductOutput>>`**

  List products, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every product).

  Types: [`ProductFilterInput`](#productfilterinput), [`ProductOutput`](#productoutput)

- **`count(): Promise<number>`**

- **`update(id: string, input: UpdateProductInput): Promise<ProductOutput>`**

  Types: [`UpdateProductInput`](#updateproductinput), [`ProductOutput`](#productoutput)

- **`delete(id: string): Promise<void>`**

- **`getBySlug(slug: string): Promise<ProductOutput | null>`**

  Types: [`ProductOutput`](#productoutput)

- **`activate(id: string): Promise<ProductOutput>`**

  Types: [`ProductOutput`](#productoutput)

- **`archive(id: string): Promise<ProductOutput>`**

  Types: [`ProductOutput`](#productoutput)

- **`search(query: string): Promise<Array<ProductOutput>>`**

  Types: [`ProductOutput`](#productoutput)

- **`getVariant(id: string): Promise<ProductVariantOutput | null>`**

  Types: [`ProductVariantOutput`](#productvariantoutput)

- **`getVariants(productId: string): Promise<Array<ProductVariantOutput>>`**

  Types: [`ProductVariantOutput`](#productvariantoutput)

- **`addVariant(productId: string, input: CreateProductVariantInput): Promise<ProductVariantOutput>`**

  Types: [`CreateProductVariantInput`](#createproductvariantinput), [`ProductVariantOutput`](#productvariantoutput)

- **`updateVariant(id: string, input: CreateProductVariantInput): Promise<ProductVariantOutput>`**

  Types: [`CreateProductVariantInput`](#createproductvariantinput), [`ProductVariantOutput`](#productvariantoutput)

- **`deleteVariant(id: string): Promise<void>`**

### commerce.customObjects

Class `CustomObjects`. Also available as `commerce.customStates`.

- **`createType(input: CreateCustomObjectTypeInput): Promise<CustomObjectTypeOutput>`**

  Types: [`CreateCustomObjectTypeInput`](#createcustomobjecttypeinput), [`CustomObjectTypeOutput`](#customobjecttypeoutput)

- **`getType(id: string): Promise<CustomObjectTypeOutput | null>`**

  Types: [`CustomObjectTypeOutput`](#customobjecttypeoutput)

- **`getTypeByHandle(handle: string): Promise<CustomObjectTypeOutput | null>`**

  Types: [`CustomObjectTypeOutput`](#customobjecttypeoutput)

- **`updateType(id: string, input: UpdateCustomObjectTypeInput): Promise<CustomObjectTypeOutput>`**

  Types: [`UpdateCustomObjectTypeInput`](#updatecustomobjecttypeinput), [`CustomObjectTypeOutput`](#customobjecttypeoutput)

- **`listTypes(filter?: CustomObjectTypeFilterInput | undefined | null): Promise<Array<CustomObjectTypeOutput>>`**

  Types: [`CustomObjectTypeFilterInput`](#customobjecttypefilterinput), [`CustomObjectTypeOutput`](#customobjecttypeoutput)

- **`deleteType(id: string): Promise<void>`**

- **`createObject(input: CreateCustomObjectInput): Promise<CustomObjectOutput>`**

  Types: [`CreateCustomObjectInput`](#createcustomobjectinput), [`CustomObjectOutput`](#customobjectoutput)

- **`getObject(id: string): Promise<CustomObjectOutput | null>`**

  Types: [`CustomObjectOutput`](#customobjectoutput)

- **`getObjectByHandle(typeHandle: string, objectHandle: string): Promise<CustomObjectOutput | null>`**

  Types: [`CustomObjectOutput`](#customobjectoutput)

- **`updateObject(id: string, input: UpdateCustomObjectInput): Promise<CustomObjectOutput>`**

  Types: [`UpdateCustomObjectInput`](#updatecustomobjectinput), [`CustomObjectOutput`](#customobjectoutput)

- **`listObjects(filter?: CustomObjectFilterInput | undefined | null): Promise<Array<CustomObjectOutput>>`**

  Types: [`CustomObjectFilterInput`](#customobjectfilterinput), [`CustomObjectOutput`](#customobjectoutput)

- **`deleteObject(id: string): Promise<void>`**

### commerce.inventory

Class `Inventory`.

- **`createItem(input: CreateInventoryItemInput): Promise<InventoryItemOutput>`**

  Types: [`CreateInventoryItemInput`](#createinventoryiteminput), [`InventoryItemOutput`](#inventoryitemoutput)

- **`getStock(sku: string): Promise<StockLevelOutput | null>`**

  Types: [`StockLevelOutput`](#stockleveloutput)

- **`adjust(sku: string, quantity: number, reason: string): Promise<void>`**

- **`reserve(sku: string, quantity: number, referenceType: string, referenceId: string, expiresInSeconds?: number | undefined | null): Promise<ReservationOutput>`**

  Types: [`ReservationOutput`](#reservationoutput)

- **`confirmReservation(reservationId: string): Promise<void>`**

- **`releaseReservation(reservationId: string): Promise<void>`**

### commerce.returns

Class `Returns`.

- **`create(input: CreateReturnInput): Promise<ReturnOutput>`**

  Types: [`CreateReturnInput`](#createreturninput), [`ReturnOutput`](#returnoutput)

- **`get(id: string): Promise<ReturnOutput | null>`**

  Types: [`ReturnOutput`](#returnoutput)

- **`approve(id: string): Promise<ReturnOutput>`**

  Types: [`ReturnOutput`](#returnoutput)

- **`reject(id: string, reason: string): Promise<ReturnOutput>`**

  Types: [`ReturnOutput`](#returnoutput)

- **`list(filter?: ReturnFilterInput | undefined | null): Promise<Array<ReturnOutput>>`**

  List returns, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every return).

  Types: [`ReturnFilterInput`](#returnfilterinput), [`ReturnOutput`](#returnoutput)

- **`count(): Promise<number>`**

- **`listForOrder(orderId: string): Promise<Array<ReturnOutput>>`**

  Types: [`ReturnOutput`](#returnoutput)

- **`listForCustomer(customerId: string): Promise<Array<ReturnOutput>>`**

  Types: [`ReturnOutput`](#returnoutput)

- **`listPending(): Promise<Array<ReturnOutput>>`**

  Types: [`ReturnOutput`](#returnoutput)

- **`markReceived(id: string): Promise<ReturnOutput>`**

  Types: [`ReturnOutput`](#returnoutput)

- **`complete(id: string): Promise<ReturnOutput>`**

  Types: [`ReturnOutput`](#returnoutput)

- **`cancel(id: string): Promise<ReturnOutput>`**

  Types: [`ReturnOutput`](#returnoutput)

- **`addTracking(id: string, trackingNumber: string): Promise<ReturnOutput>`**

  Types: [`ReturnOutput`](#returnoutput)

### commerce.giftCards

Class `GiftCards`.

- **`isSupported(): Promise<boolean>`**

  Whether the gift-cards backend is available on this engine build.

- **`create(input: CreateGiftCardInput): Promise<GiftCardOutput>`**

  Types: [`CreateGiftCardInput`](#creategiftcardinput), [`GiftCardOutput`](#giftcardoutput)

- **`get(id: string): Promise<GiftCardOutput | null>`**

  Types: [`GiftCardOutput`](#giftcardoutput)

- **`getByCode(code: string): Promise<GiftCardOutput | null>`**

  Types: [`GiftCardOutput`](#giftcardoutput)

- **`update(id: string, input: UpdateGiftCardInput): Promise<GiftCardOutput>`**

  Types: [`UpdateGiftCardInput`](#updategiftcardinput), [`GiftCardOutput`](#giftcardoutput)

- **`list(filter?: GiftCardFilterInput | undefined | null): Promise<Array<GiftCardOutput>>`**

  Types: [`GiftCardFilterInput`](#giftcardfilterinput), [`GiftCardOutput`](#giftcardoutput)

- **`charge(id: string, amount: string, referenceId?: string | undefined | null): Promise<GiftCardTransactionOutput>`**

  Types: [`GiftCardTransactionOutput`](#giftcardtransactionoutput)

- **`refund(id: string, amount: string, referenceId?: string | undefined | null): Promise<GiftCardTransactionOutput>`**

  Types: [`GiftCardTransactionOutput`](#giftcardtransactionoutput)

- **`disable(id: string): Promise<GiftCardOutput>`**

  Types: [`GiftCardOutput`](#giftcardoutput)

- **`getTransactions(giftCardId: string): Promise<Array<GiftCardTransactionOutput>>`**

  Types: [`GiftCardTransactionOutput`](#giftcardtransactionoutput)

### commerce.loyalty

Class `Loyalty`.

- **`isSupported(): Promise<boolean>`**

  Whether the loyalty backend is available on this engine build.

- **`createProgram(input: CreateLoyaltyProgramInput): Promise<LoyaltyProgramOutput>`**

  Types: [`CreateLoyaltyProgramInput`](#createloyaltyprograminput), [`LoyaltyProgramOutput`](#loyaltyprogramoutput)

- **`getProgram(id: string): Promise<LoyaltyProgramOutput | null>`**

  Types: [`LoyaltyProgramOutput`](#loyaltyprogramoutput)

- **`listPrograms(): Promise<Array<LoyaltyProgramOutput>>`**

  Types: [`LoyaltyProgramOutput`](#loyaltyprogramoutput)

- **`enroll(input: EnrollCustomerInput): Promise<LoyaltyAccountOutput>`**

  Types: [`EnrollCustomerInput`](#enrollcustomerinput), [`LoyaltyAccountOutput`](#loyaltyaccountoutput)

- **`getAccount(id: string): Promise<LoyaltyAccountOutput | null>`**

  Types: [`LoyaltyAccountOutput`](#loyaltyaccountoutput)

- **`getAccountByCustomer(customerId: string, programId: string): Promise<LoyaltyAccountOutput | null>`**

  Types: [`LoyaltyAccountOutput`](#loyaltyaccountoutput)

- **`listAccounts(filter?: LoyaltyAccountFilterInput | undefined | null): Promise<Array<LoyaltyAccountOutput>>`**

  Types: [`LoyaltyAccountFilterInput`](#loyaltyaccountfilterinput), [`LoyaltyAccountOutput`](#loyaltyaccountoutput)

- **`adjustPoints(input: AdjustPointsInput): Promise<LoyaltyTransactionOutput>`**

  Types: [`AdjustPointsInput`](#adjustpointsinput), [`LoyaltyTransactionOutput`](#loyaltytransactionoutput)

- **`getTransactions(accountId: string, limit?: number | undefined | null): Promise<Array<LoyaltyTransactionOutput>>`**

  Types: [`LoyaltyTransactionOutput`](#loyaltytransactionoutput)

- **`createReward(input: CreateRewardInput): Promise<RewardOutput>`**

  Types: [`CreateRewardInput`](#createrewardinput), [`RewardOutput`](#rewardoutput)

- **`getReward(id: string): Promise<RewardOutput | null>`**

  Types: [`RewardOutput`](#rewardoutput)

- **`listRewards(filter?: RewardFilterInput | undefined | null): Promise<Array<RewardOutput>>`**

  Types: [`RewardFilterInput`](#rewardfilterinput), [`RewardOutput`](#rewardoutput)

- **`deleteReward(id: string): Promise<void>`**

### commerce.storeCredits

Class `StoreCredits`.

- **`isSupported(): Promise<boolean>`**

  Whether the store-credits backend is available on this engine build.

- **`create(input: CreateStoreCreditInput): Promise<StoreCreditOutput>`**

  Types: [`CreateStoreCreditInput`](#createstorecreditinput), [`StoreCreditOutput`](#storecreditoutput)

- **`get(id: string): Promise<StoreCreditOutput | null>`**

  Types: [`StoreCreditOutput`](#storecreditoutput)

- **`list(filter?: StoreCreditFilterInput | undefined | null): Promise<Array<StoreCreditOutput>>`**

  Types: [`StoreCreditFilterInput`](#storecreditfilterinput), [`StoreCreditOutput`](#storecreditoutput)

- **`adjust(id: string, input: AdjustStoreCreditInput): Promise<StoreCreditOutput>`**

  Types: [`AdjustStoreCreditInput`](#adjuststorecreditinput), [`StoreCreditOutput`](#storecreditoutput)

- **`apply(id: string, amount: string, referenceId?: string | undefined | null): Promise<StoreCreditTransactionOutput>`**

  Apply (redeem) an amount from the credit, returning the ledger transaction.

  Types: [`StoreCreditTransactionOutput`](#storecredittransactionoutput)

- **`getTransactions(storeCreditId: string): Promise<Array<StoreCreditTransactionOutput>>`**

  Types: [`StoreCreditTransactionOutput`](#storecredittransactionoutput)

### commerce.reviews

Class `Reviews`.

- **`isSupported(): Promise<boolean>`**

  Whether the reviews backend is available on this engine build.

- **`create(input: CreateReviewInput): Promise<ReviewOutput>`**

  Types: [`CreateReviewInput`](#createreviewinput), [`ReviewOutput`](#reviewoutput)

- **`get(id: string): Promise<ReviewOutput | null>`**

  Types: [`ReviewOutput`](#reviewoutput)

- **`update(id: string, input: UpdateReviewInput): Promise<ReviewOutput>`**

  Types: [`UpdateReviewInput`](#updatereviewinput), [`ReviewOutput`](#reviewoutput)

- **`list(filter?: ReviewFilterInput | undefined | null): Promise<Array<ReviewOutput>>`**

  Types: [`ReviewFilterInput`](#reviewfilterinput), [`ReviewOutput`](#reviewoutput)

- **`delete(id: string): Promise<void>`**

- **`getSummary(productId: string): Promise<ReviewSummaryOutput>`**

  Aggregate rating summary for a product (average, total, star distribution).

  Types: [`ReviewSummaryOutput`](#reviewsummaryoutput)

- **`markHelpful(id: string): Promise<void>`**

- **`markReported(id: string): Promise<void>`**

### commerce.wishlists

Class `Wishlists`.

- **`isSupported(): Promise<boolean>`**

  Whether the wishlists backend is available on this engine build.

- **`create(input: CreateWishlistInput): Promise<WishlistOutput>`**

  Types: [`CreateWishlistInput`](#createwishlistinput), [`WishlistOutput`](#wishlistoutput)

- **`get(id: string): Promise<WishlistOutput | null>`**

  Types: [`WishlistOutput`](#wishlistoutput)

- **`update(id: string, input: UpdateWishlistInput): Promise<WishlistOutput>`**

  Types: [`UpdateWishlistInput`](#updatewishlistinput), [`WishlistOutput`](#wishlistoutput)

- **`list(filter?: WishlistFilterInput | undefined | null): Promise<Array<WishlistOutput>>`**

  Types: [`WishlistFilterInput`](#wishlistfilterinput), [`WishlistOutput`](#wishlistoutput)

- **`delete(id: string): Promise<void>`**

- **`addItem(wishlistId: string, item: AddWishlistItemInput): Promise<WishlistItemOutput>`**

  Add a product to a wishlist, returning the added item.

  Types: [`AddWishlistItemInput`](#addwishlistiteminput), [`WishlistItemOutput`](#wishlistitemoutput)

- **`removeItem(wishlistId: string, productId: string): Promise<void>`**

### commerce.segments

Class `Segments`.

- **`isSupported(): Promise<boolean>`**

  Whether the segments backend is available on this engine build.

- **`create(input: CreateSegmentInput): Promise<SegmentOutput>`**

  Types: [`CreateSegmentInput`](#createsegmentinput), [`SegmentOutput`](#segmentoutput)

- **`get(id: string): Promise<SegmentOutput | null>`**

  Types: [`SegmentOutput`](#segmentoutput)

- **`update(id: string, input: UpdateSegmentInput): Promise<SegmentOutput>`**

  Types: [`UpdateSegmentInput`](#updatesegmentinput), [`SegmentOutput`](#segmentoutput)

- **`list(filter?: SegmentFilterInput | undefined | null): Promise<Array<SegmentOutput>>`**

  Types: [`SegmentFilterInput`](#segmentfilterinput), [`SegmentOutput`](#segmentoutput)

- **`delete(id: string): Promise<void>`**

- **`addMember(segmentId: string, customerId: string): Promise<SegmentMembershipOutput>`**

  Add a customer to a (static) segment, returning the membership record.

  Types: [`SegmentMembershipOutput`](#segmentmembershipoutput)

- **`removeMember(segmentId: string, customerId: string): Promise<void>`**

- **`listMembers(segmentId: string, limit?: number | undefined | null, offset?: number | undefined | null): Promise<Array<SegmentMembershipOutput>>`**

  Types: [`SegmentMembershipOutput`](#segmentmembershipoutput)

- **`isMember(segmentId: string, customerId: string): Promise<boolean>`**

### commerce.payments

Class `Payments`.

- **`create(input: CreatePaymentInput): Promise<PaymentOutput>`**

  Types: [`CreatePaymentInput`](#createpaymentinput), [`PaymentOutput`](#paymentoutput)

- **`createExact(input: CreatePaymentExactInput): Promise<PaymentOutput>`**

  Create a payment without any floating-point conversion.

  Types: [`CreatePaymentExactInput`](#createpaymentexactinput), [`PaymentOutput`](#paymentoutput)

- **`get(id: string): Promise<PaymentOutput | null>`**

  Types: [`PaymentOutput`](#paymentoutput)

- **`list(filter?: PaymentFilterInput | undefined | null): Promise<Array<PaymentOutput>>`**

  List payments, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every payment).

  Types: [`PaymentFilterInput`](#paymentfilterinput), [`PaymentOutput`](#paymentoutput)

- **`markCompleted(id: string): Promise<PaymentOutput>`**

  Types: [`PaymentOutput`](#paymentoutput)

- **`markFailed(id: string, reason: string, code?: string | undefined | null): Promise<PaymentOutput>`**

  Types: [`PaymentOutput`](#paymentoutput)

- **`cancel(id: string): Promise<PaymentOutput>`**

  Types: [`PaymentOutput`](#paymentoutput)

- **`createRefund(input: CreateRefundInput): Promise<RefundOutput>`**

  Types: [`CreateRefundInput`](#createrefundinput), [`RefundOutput`](#refundoutput)

- **`createRefundExact(input: CreateRefundExactInput): Promise<RefundOutput>`**

  Create a refund without any floating-point conversion.

  Types: [`CreateRefundExactInput`](#createrefundexactinput), [`RefundOutput`](#refundoutput)

- **`count(): Promise<number>`**

### commerce.x402

Class `X402`.

- **`createIntent(input: X402CreateIntentInput): Promise<X402IntentOutput>`**

  Types: [`X402CreateIntentInput`](#x402createintentinput), [`X402IntentOutput`](#x402intentoutput)

- **`signIntent(intentId: string, input: X402SignIntentInput): Promise<X402IntentOutput>`**

  Types: [`X402SignIntentInput`](#x402signintentinput), [`X402IntentOutput`](#x402intentoutput)

- **`getIntent(id: string): Promise<X402IntentOutput | null>`**

  Types: [`X402IntentOutput`](#x402intentoutput)

- **`listIntents(filter: X402IntentFilterInput): Promise<Array<X402IntentOutput>>`**

  Types: [`X402IntentFilterInput`](#x402intentfilterinput), [`X402IntentOutput`](#x402intentoutput)

- **`markSettled(intentId: string, txHash: string, blockNumber: number): Promise<X402IntentOutput>`**

  Types: [`X402IntentOutput`](#x402intentoutput)

- **`getNextNonce(payerAddress: string): Promise<number>`**

- **`registerAgent(input: X402AgentCardInput): Promise<X402AgentCardOutput>`**

  Types: [`X402AgentCardInput`](#x402agentcardinput), [`X402AgentCardOutput`](#x402agentcardoutput)

- **`discoverAgents(network?: X402NetworkInput, asset?: X402AssetInput, skill?: X402A2ASkillInput, trustLevel?: X402TrustLevelInput): Promise<Array<X402AgentCardOutput>>`**

  Types: [`X402NetworkInput`](#x402networkinput), [`X402AssetInput`](#x402assetinput), [`X402A2ASkillInput`](#x402a2askillinput), [`X402TrustLevelInput`](#x402trustlevelinput), [`X402AgentCardOutput`](#x402agentcardoutput)

- **`getAgent(id: string): Promise<X402AgentCardOutput | null>`**

  Types: [`X402AgentCardOutput`](#x402agentcardoutput)

- **`getAgentByWallet(walletAddress: string): Promise<X402AgentCardOutput | null>`**

  Types: [`X402AgentCardOutput`](#x402agentcardoutput)

- **`verifyAgent(id: string): Promise<X402AgentCardOutput>`**

  Types: [`X402AgentCardOutput`](#x402agentcardoutput)

- **`listAgents(filter: X402AgentCardFilterInput): Promise<Array<X402AgentCardOutput>>`**

  Types: [`X402AgentCardFilterInput`](#x402agentcardfilterinput), [`X402AgentCardOutput`](#x402agentcardoutput)

- **`getCreditBalance(input: X402CreditBalanceInput): Promise<number>`**

  Types: [`X402CreditBalanceInput`](#x402creditbalanceinput)

- **`getCreditAccount(input: X402CreditBalanceInput): Promise<X402CreditAccountOutput | null>`**

  Types: [`X402CreditBalanceInput`](#x402creditbalanceinput), [`X402CreditAccountOutput`](#x402creditaccountoutput)

- **`creditAccount(input: X402CreditAdjustmentInput): Promise<X402CreditTransactionOutput>`**

  Types: [`X402CreditAdjustmentInput`](#x402creditadjustmentinput), [`X402CreditTransactionOutput`](#x402credittransactionoutput)

- **`debitAccount(input: X402CreditAdjustmentInput): Promise<X402CreditTransactionOutput>`**

  Types: [`X402CreditAdjustmentInput`](#x402creditadjustmentinput), [`X402CreditTransactionOutput`](#x402credittransactionoutput)

- **`listCreditTransactions(filter: X402CreditTransactionFilterInput): Promise<Array<X402CreditTransactionOutput>>`**

  Types: [`X402CreditTransactionFilterInput`](#x402credittransactionfilterinput), [`X402CreditTransactionOutput`](#x402credittransactionoutput)

### commerce.shipments

Class `Shipments`.

- **`create(input: CreateShipmentInput): Promise<ShipmentOutput>`**

  Types: [`CreateShipmentInput`](#createshipmentinput), [`ShipmentOutput`](#shipmentoutput)

- **`get(id: string): Promise<ShipmentOutput | null>`**

  Types: [`ShipmentOutput`](#shipmentoutput)

- **`list(filter?: ShipmentFilterInput | undefined | null): Promise<Array<ShipmentOutput>>`**

  List shipments, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every shipment).

  Types: [`ShipmentFilterInput`](#shipmentfilterinput), [`ShipmentOutput`](#shipmentoutput)

- **`ship(id: string, trackingNumber?: string | undefined | null): Promise<ShipmentOutput>`**

  Types: [`ShipmentOutput`](#shipmentoutput)

- **`deliver(id: string): Promise<ShipmentOutput>`**

  Types: [`ShipmentOutput`](#shipmentoutput)

- **`cancel(id: string): Promise<ShipmentOutput>`**

  Types: [`ShipmentOutput`](#shipmentoutput)

- **`count(): Promise<number>`**

### commerce.warranties

Class `Warranties`.

- **`create(input: CreateWarrantyInput): Promise<WarrantyOutput>`**

  Types: [`CreateWarrantyInput`](#createwarrantyinput), [`WarrantyOutput`](#warrantyoutput)

- **`get(id: string): Promise<WarrantyOutput | null>`**

  Types: [`WarrantyOutput`](#warrantyoutput)

- **`list(filter?: WarrantyFilterInput | undefined | null): Promise<Array<WarrantyOutput>>`**

  List warranties, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every warranty).

  Types: [`WarrantyFilterInput`](#warrantyfilterinput), [`WarrantyOutput`](#warrantyoutput)

- **`createClaim(input: CreateWarrantyClaimInput): Promise<WarrantyClaimOutput>`**

  Types: [`CreateWarrantyClaimInput`](#createwarrantyclaiminput), [`WarrantyClaimOutput`](#warrantyclaimoutput)

- **`approveClaim(id: string): Promise<WarrantyClaimOutput>`**

  Types: [`WarrantyClaimOutput`](#warrantyclaimoutput)

- **`denyClaim(id: string, reason: string): Promise<WarrantyClaimOutput>`**

  Types: [`WarrantyClaimOutput`](#warrantyclaimoutput)

- **`completeClaim(id: string, resolution: WarrantyClaimResolutionInput): Promise<WarrantyClaimOutput>`**

  An unrecognised resolution is refused with `VALIDATION`.

  Types: [`WarrantyClaimResolutionInput`](#warrantyclaimresolutioninput), [`WarrantyClaimOutput`](#warrantyclaimoutput)

- **`count(): Promise<number>`**

### commerce.purchaseOrders

Class `PurchaseOrders`.

- **`createSupplier(input: CreateSupplierInput): Promise<SupplierOutput>`**

  Types: [`CreateSupplierInput`](#createsupplierinput), [`SupplierOutput`](#supplieroutput)

- **`getSupplier(id: string): Promise<SupplierOutput | null>`**

  Types: [`SupplierOutput`](#supplieroutput)

- **`listSuppliers(filter?: SupplierFilterInput | undefined | null): Promise<Array<SupplierOutput>>`**

  List suppliers, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (server default page size).

  Types: [`SupplierFilterInput`](#supplierfilterinput), [`SupplierOutput`](#supplieroutput)

- **`create(input: CreatePurchaseOrderInput): Promise<PurchaseOrderOutput>`**

  Types: [`CreatePurchaseOrderInput`](#createpurchaseorderinput), [`PurchaseOrderOutput`](#purchaseorderoutput)

- **`get(id: string): Promise<PurchaseOrderOutput | null>`**

  Types: [`PurchaseOrderOutput`](#purchaseorderoutput)

- **`list(filter?: PurchaseOrderFilterInput | undefined | null): Promise<Array<PurchaseOrderOutput>>`**

  List purchase orders, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (server default page size).

  Types: [`PurchaseOrderFilterInput`](#purchaseorderfilterinput), [`PurchaseOrderOutput`](#purchaseorderoutput)

- **`submit(id: string): Promise<PurchaseOrderOutput>`**

  Types: [`PurchaseOrderOutput`](#purchaseorderoutput)

- **`approve(id: string, approvedBy: string): Promise<PurchaseOrderOutput>`**

  Types: [`PurchaseOrderOutput`](#purchaseorderoutput)

- **`send(id: string): Promise<PurchaseOrderOutput>`**

  Types: [`PurchaseOrderOutput`](#purchaseorderoutput)

- **`cancel(id: string): Promise<PurchaseOrderOutput>`**

  Types: [`PurchaseOrderOutput`](#purchaseorderoutput)

- **`count(): Promise<number>`**

### commerce.invoices

Class `Invoices`.

- **`create(input: CreateInvoiceInput): Promise<InvoiceOutput>`**

  Types: [`CreateInvoiceInput`](#createinvoiceinput), [`InvoiceOutput`](#invoiceoutput)

- **`get(id: string): Promise<InvoiceOutput | null>`**

  Types: [`InvoiceOutput`](#invoiceoutput)

- **`list(filter?: InvoiceFilterInput | undefined | null): Promise<Array<InvoiceOutput>>`**

  List invoices, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every invoice).

  Types: [`InvoiceFilterInput`](#invoicefilterinput), [`InvoiceOutput`](#invoiceoutput)

- **`send(id: string): Promise<InvoiceOutput>`**

  Types: [`InvoiceOutput`](#invoiceoutput)

- **`void(id: string): Promise<InvoiceOutput>`**

  Types: [`InvoiceOutput`](#invoiceoutput)

- **`recordPayment(id: string, input: RecordPaymentInput): Promise<InvoiceOutput>`**

  Types: [`RecordPaymentInput`](#recordpaymentinput), [`InvoiceOutput`](#invoiceoutput)

- **`getOverdue(): Promise<Array<InvoiceOutput>>`**

  Types: [`InvoiceOutput`](#invoiceoutput)

- **`count(): Promise<number>`**

### commerce.bom

Class `Bom`.

- **`create(input: CreateBomInput): Promise<BomOutput>`**

  Types: [`CreateBomInput`](#createbominput), [`BomOutput`](#bomoutput)

- **`get(id: string): Promise<BomOutput | null>`**

  Types: [`BomOutput`](#bomoutput)

- **`list(filter?: BomFilterInput | undefined | null): Promise<Array<BomOutput>>`**

  List BOMs, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every BOM).

  Types: [`BomFilterInput`](#bomfilterinput), [`BomOutput`](#bomoutput)

- **`addComponent(bomId: string, input: CreateBomComponentInput): Promise<BomComponentOutput>`**

  Types: [`CreateBomComponentInput`](#createbomcomponentinput), [`BomComponentOutput`](#bomcomponentoutput)

- **`getComponents(bomId: string): Promise<Array<BomComponentOutput>>`**

  Types: [`BomComponentOutput`](#bomcomponentoutput)

- **`activate(id: string): Promise<BomOutput>`**

  Types: [`BomOutput`](#bomoutput)

- **`count(): Promise<number>`**

### commerce.workOrders

Class `WorkOrders`.

- **`create(input: CreateWorkOrderInput): Promise<WorkOrderOutput>`**

  Types: [`CreateWorkOrderInput`](#createworkorderinput), [`WorkOrderOutput`](#workorderoutput)

- **`get(id: string): Promise<WorkOrderOutput | null>`**

  Types: [`WorkOrderOutput`](#workorderoutput)

- **`list(filter?: WorkOrderFilterInput | undefined | null): Promise<Array<WorkOrderOutput>>`**

  List work orders, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (server default page size).

  Types: [`WorkOrderFilterInput`](#workorderfilterinput), [`WorkOrderOutput`](#workorderoutput)

- **`start(id: string): Promise<WorkOrderOutput>`**

  Types: [`WorkOrderOutput`](#workorderoutput)

- **`complete(id: string, quantityCompleted: number): Promise<WorkOrderOutput>`**

  Types: [`WorkOrderOutput`](#workorderoutput)

- **`cancel(id: string): Promise<WorkOrderOutput>`**

  Types: [`WorkOrderOutput`](#workorderoutput)

- **`count(): Promise<number>`**

### commerce.carts

Class `Carts`.

- **`create(input: CreateCartInput): Promise<CartOutput>`**

  Create a new cart

  Types: [`CreateCartInput`](#createcartinput), [`CartOutput`](#cartoutput)

- **`get(id: string): Promise<CartOutput | null>`**

  Get a cart by ID

  Types: [`CartOutput`](#cartoutput)

- **`getByNumber(cartNumber: string): Promise<CartOutput | null>`**

  Get a cart by cart number

  Types: [`CartOutput`](#cartoutput)

- **`update(id: string, input: UpdateCartInput): Promise<CartOutput>`**

  Update a cart

  Types: [`UpdateCartInput`](#updatecartinput), [`CartOutput`](#cartoutput)

- **`list(filter?: CartFilterInput | undefined | null): Promise<Array<CartOutput>>`**

  List carts, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (every cart).

  Types: [`CartFilterInput`](#cartfilterinput), [`CartOutput`](#cartoutput)

- **`forCustomer(customerId: string): Promise<Array<CartOutput>>`**

  List carts for a customer

  Types: [`CartOutput`](#cartoutput)

- **`delete(id: string): Promise<void>`**

  Delete a cart

- **`addItem(cartId: string, item: AddCartItemInput): Promise<CartItemOutput>`**

  Add an item to the cart

  Types: [`AddCartItemInput`](#addcartiteminput), [`CartItemOutput`](#cartitemoutput)

- **`addItemExact(cartId: string, item: AddCartItemExactInput): Promise<CartItemOutput>`**

  Add a cart item without any floating-point conversion.

  Types: [`AddCartItemExactInput`](#addcartitemexactinput), [`CartItemOutput`](#cartitemoutput)

- **`updateItem(itemId: string, input: UpdateCartItemInput): Promise<CartItemOutput>`**

  Update a cart item

  Types: [`UpdateCartItemInput`](#updatecartiteminput), [`CartItemOutput`](#cartitemoutput)

- **`removeItem(itemId: string): Promise<void>`**

  Remove an item from the cart

- **`getItems(cartId: string): Promise<Array<CartItemOutput>>`**

  Get items in a cart

  Types: [`CartItemOutput`](#cartitemoutput)

- **`clearItems(cartId: string): Promise<void>`**

  Clear all items from the cart

- **`setShippingAddress(id: string, address: CartAddressInput): Promise<CartOutput>`**

  Set the shipping address

  Types: [`CartAddressInput`](#cartaddressinput), [`CartOutput`](#cartoutput)

- **`setShipping(id: string, input: SetCartShippingInput): Promise<CartOutput>`**

  Set shipping selection (address + method/carrier/amount)

  Types: [`SetCartShippingInput`](#setcartshippinginput), [`CartOutput`](#cartoutput)

- **`setBillingAddress(id: string, address: CartAddressInput): Promise<CartOutput>`**

  Set the billing address

  Types: [`CartAddressInput`](#cartaddressinput), [`CartOutput`](#cartoutput)

- **`getShippingRates(id: string): Promise<Array<ShippingRateOutput>>`**

  Get available shipping rates

  Types: [`ShippingRateOutput`](#shippingrateoutput)

- **`setPayment(id: string, input: SetCartPaymentInput): Promise<CartOutput>`**

  Set payment method

  Types: [`SetCartPaymentInput`](#setcartpaymentinput), [`CartOutput`](#cartoutput)

- **`applyDiscount(id: string, couponCode: string): Promise<CartOutput>`**

  Apply a discount/coupon code

  Types: [`CartOutput`](#cartoutput)

- **`removeDiscount(id: string): Promise<CartOutput>`**

  Remove discount from cart

  Types: [`CartOutput`](#cartoutput)

- **`markReadyForPayment(id: string): Promise<CartOutput>`**

  Mark cart as ready for payment

  Types: [`CartOutput`](#cartoutput)

- **`beginCheckout(id: string): Promise<CartOutput>`**

  Begin checkout process

  Types: [`CartOutput`](#cartoutput)

- **`complete(id: string): Promise<CheckoutResultOutput>`**

  Complete checkout and create order

  Types: [`CheckoutResultOutput`](#checkoutresultoutput)

- **`cancel(id: string): Promise<CartOutput>`**

  Cancel a cart

  Types: [`CartOutput`](#cartoutput)

- **`abandon(id: string): Promise<CartOutput>`**

  Mark cart as abandoned

  Types: [`CartOutput`](#cartoutput)

- **`expire(id: string): Promise<CartOutput>`**

  Mark cart as expired

  Types: [`CartOutput`](#cartoutput)

- **`reserveInventory(id: string): Promise<CartOutput>`**

  Reserve inventory for cart items

  Types: [`CartOutput`](#cartoutput)

- **`releaseInventory(id: string): Promise<CartOutput>`**

  Release reserved inventory for cart items

  Types: [`CartOutput`](#cartoutput)

- **`recalculate(id: string): Promise<CartOutput>`**

  Recalculate cart totals

  Types: [`CartOutput`](#cartoutput)

- **`setTax(id: string, taxAmount?: number | undefined | null, taxAmountExact?: string | undefined | null): Promise<CartOutput>`**

  Set tax amount.

  `tax_amount_exact` is the exact base-10 form and wins when present; the
  `f64` is what callers sent before it existed and still works alone.

  Types: [`CartOutput`](#cartoutput)

- **`getAbandoned(): Promise<Array<CartOutput>>`**

  Get abandoned carts

  Types: [`CartOutput`](#cartoutput)

- **`getExpired(): Promise<Array<CartOutput>>`**

  Get expired carts

  Types: [`CartOutput`](#cartoutput)

- **`count(): Promise<number>`**

  Count carts

### commerce.analytics

Class `Analytics`.

Analytics and forecasting API

- **`salesSummary(query?: AnalyticsQueryInput | undefined | null): Promise<SalesSummaryOutput>`**

  Get sales summary for a time period

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`SalesSummaryOutput`](#salessummaryoutput)

- **`revenueByPeriod(query?: AnalyticsQueryInput | undefined | null): Promise<Array<RevenueByPeriodOutput>>`**

  Get revenue broken down by time periods

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`RevenueByPeriodOutput`](#revenuebyperiodoutput)

- **`topProducts(query?: AnalyticsQueryInput | undefined | null): Promise<Array<TopProductOutput>>`**

  Get top selling products

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`TopProductOutput`](#topproductoutput)

- **`productPerformance(query?: AnalyticsQueryInput | undefined | null): Promise<Array<ProductPerformanceOutput>>`**

  Get product performance with period comparison

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`ProductPerformanceOutput`](#productperformanceoutput)

- **`customerMetrics(query?: AnalyticsQueryInput | undefined | null): Promise<CustomerMetricsOutput>`**

  Get customer metrics

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`CustomerMetricsOutput`](#customermetricsoutput)

- **`topCustomers(query?: AnalyticsQueryInput | undefined | null): Promise<Array<TopCustomerOutput>>`**

  Get top customers by spend

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`TopCustomerOutput`](#topcustomeroutput)

- **`inventoryHealth(): Promise<InventoryHealthOutput>`**

  Get inventory health summary

  Types: [`InventoryHealthOutput`](#inventoryhealthoutput)

- **`lowStockItems(threshold?: number | undefined | null): Promise<Array<LowStockItemOutput>>`**

  Get low stock items

  Types: [`LowStockItemOutput`](#lowstockitemoutput)

- **`inventoryMovement(query?: AnalyticsQueryInput | undefined | null): Promise<Array<InventoryMovementOutput>>`**

  Get inventory movement summary

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`InventoryMovementOutput`](#inventorymovementoutput)

- **`demandForecast(skus?: Array<string> | undefined | null, daysAhead?: number | undefined | null): Promise<Array<DemandForecastOutput>>`**

  Get demand forecast for inventory items

  Types: [`DemandForecastOutput`](#demandforecastoutput)

- **`revenueForecast(periodsAhead?: number | undefined | null, granularity?: AnalyticsGranularity | undefined | null): Promise<Array<RevenueForecastOutput>>`**

  Get revenue forecast. `granularity` defaults to `month`.

  Types: [`AnalyticsGranularity`](#analyticsgranularity), [`RevenueForecastOutput`](#revenueforecastoutput)

- **`orderStatusBreakdown(query?: AnalyticsQueryInput | undefined | null): Promise<OrderStatusBreakdownOutput>`**

  Get order status breakdown

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`OrderStatusBreakdownOutput`](#orderstatusbreakdownoutput)

- **`fulfillmentMetrics(query?: AnalyticsQueryInput | undefined | null): Promise<FulfillmentMetricsOutput>`**

  Get fulfillment metrics

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`FulfillmentMetricsOutput`](#fulfillmentmetricsoutput)

- **`returnMetrics(query?: AnalyticsQueryInput | undefined | null): Promise<ReturnMetricsOutput>`**

  Get return metrics

  Types: [`AnalyticsQueryInput`](#analyticsqueryinput), [`ReturnMetricsOutput`](#returnmetricsoutput)

### commerce.currency

Class `CurrencyOperations`.

Currency and exchange rate operations API

- **`getRate(from: string, to: string): Promise<ExchangeRateOutput | null>`**

  Get exchange rate between two currencies

  Types: [`ExchangeRateOutput`](#exchangerateoutput)

- **`getRatesFor(baseCurrency: string): Promise<Array<ExchangeRateOutput>>`**

  Get all exchange rates for a base currency

  Types: [`ExchangeRateOutput`](#exchangerateoutput)

- **`listRates(filter?: ExchangeRateFilterInput | undefined | null): Promise<Array<ExchangeRateOutput>>`**

  List exchange rates with optional filtering

  Types: [`ExchangeRateFilterInput`](#exchangeratefilterinput), [`ExchangeRateOutput`](#exchangerateoutput)

- **`setRate(input: SetExchangeRateInput): Promise<ExchangeRateOutput>`**

  Set an exchange rate

  Types: [`SetExchangeRateInput`](#setexchangerateinput), [`ExchangeRateOutput`](#exchangerateoutput)

- **`setRates(inputs: Array<SetExchangeRateInput>): Promise<Array<ExchangeRateOutput>>`**

  Set multiple exchange rates at once

  Types: [`SetExchangeRateInput`](#setexchangerateinput), [`ExchangeRateOutput`](#exchangerateoutput)

- **`deleteRate(id: string): Promise<boolean>`**

  Delete an exchange rate by ID

- **`convert(input: ConvertCurrencyInput): Promise<ConversionResultOutput>`**

  Convert an amount from one currency to another

  Types: [`ConvertCurrencyInput`](#convertcurrencyinput), [`ConversionResultOutput`](#conversionresultoutput)

- **`getSettings(): Promise<StoreCurrencySettingsOutput>`**

  Get store currency settings

  Types: [`StoreCurrencySettingsOutput`](#storecurrencysettingsoutput)

- **`updateSettings(input: StoreCurrencySettingsInput): Promise<StoreCurrencySettingsOutput>`**

  Update store currency settings

  Types: [`StoreCurrencySettingsInput`](#storecurrencysettingsinput), [`StoreCurrencySettingsOutput`](#storecurrencysettingsoutput)

- **`setBaseCurrency(currencyCode: string): Promise<StoreCurrencySettingsOutput>`**

  Set the store's base currency

  Types: [`StoreCurrencySettingsOutput`](#storecurrencysettingsoutput)

- **`enableCurrencies(currencyCodes: Array<string>): Promise<StoreCurrencySettingsOutput>`**

  Enable currencies for the store

  Types: [`StoreCurrencySettingsOutput`](#storecurrencysettingsoutput)

- **`isEnabled(currencyCode: string): Promise<boolean>`**

  Check if a currency is enabled

- **`getBaseCurrency(): Promise<string>`**

  Get the store's base currency

- **`getEnabledCurrencies(): Promise<Array<string>>`**

  Get all enabled currencies

- **`format(amount: number, currencyCode: string): Promise<string>`**

  Format an amount with currency symbol

### commerce.subscriptions

Class `Subscriptions`.

- **`createPlan(input: CreateSubscriptionPlanInput): Promise<SubscriptionPlanOutput>`**

  Create a new subscription plan

  Types: [`CreateSubscriptionPlanInput`](#createsubscriptionplaninput), [`SubscriptionPlanOutput`](#subscriptionplanoutput)

- **`getPlan(id: string): Promise<SubscriptionPlanOutput | null>`**

  Get a subscription plan by ID

  Types: [`SubscriptionPlanOutput`](#subscriptionplanoutput)

- **`getPlanByCode(code: string): Promise<SubscriptionPlanOutput | null>`**

  Get a subscription plan by code

  Types: [`SubscriptionPlanOutput`](#subscriptionplanoutput)

- **`listPlans(filter?: SubscriptionPlanFilterInput | undefined | null): Promise<Array<SubscriptionPlanOutput>>`**

  List subscription plans

  Types: [`SubscriptionPlanFilterInput`](#subscriptionplanfilterinput), [`SubscriptionPlanOutput`](#subscriptionplanoutput)

- **`updatePlan(id: string, input: UpdateSubscriptionPlanInput): Promise<SubscriptionPlanOutput>`**

  Update a subscription plan

  Types: [`UpdateSubscriptionPlanInput`](#updatesubscriptionplaninput), [`SubscriptionPlanOutput`](#subscriptionplanoutput)

- **`activatePlan(id: string): Promise<SubscriptionPlanOutput>`**

  Activate a subscription plan

  Types: [`SubscriptionPlanOutput`](#subscriptionplanoutput)

- **`archivePlan(id: string): Promise<SubscriptionPlanOutput>`**

  Archive a subscription plan

  Types: [`SubscriptionPlanOutput`](#subscriptionplanoutput)

- **`subscribe(input: CreateSubscriptionInput): Promise<SubscriptionOutput>`**

  Create a subscription for a customer

  Types: [`CreateSubscriptionInput`](#createsubscriptioninput), [`SubscriptionOutput`](#subscriptionoutput)

- **`get(id: string): Promise<SubscriptionOutput | null>`**

  Get a subscription by ID

  Types: [`SubscriptionOutput`](#subscriptionoutput)

- **`getByNumber(number: string): Promise<SubscriptionOutput | null>`**

  Get a subscription by number

  Types: [`SubscriptionOutput`](#subscriptionoutput)

- **`list(filter?: SubscriptionFilterInput | undefined | null): Promise<Array<SubscriptionOutput>>`**

  List subscriptions

  Types: [`SubscriptionFilterInput`](#subscriptionfilterinput), [`SubscriptionOutput`](#subscriptionoutput)

- **`update(id: string, input: UpdateSubscriptionInput): Promise<SubscriptionOutput>`**

  Update a subscription

  Types: [`UpdateSubscriptionInput`](#updatesubscriptioninput), [`SubscriptionOutput`](#subscriptionoutput)

- **`pause(id: string, input?: PauseSubscriptionInput | undefined | null): Promise<SubscriptionOutput>`**

  Pause a subscription

  Types: [`PauseSubscriptionInput`](#pausesubscriptioninput), [`SubscriptionOutput`](#subscriptionoutput)

- **`resume(id: string): Promise<SubscriptionOutput>`**

  Resume a paused subscription

  Types: [`SubscriptionOutput`](#subscriptionoutput)

- **`cancel(id: string, input?: CancelSubscriptionInput | undefined | null): Promise<SubscriptionOutput>`**

  Cancel a subscription

  Types: [`CancelSubscriptionInput`](#cancelsubscriptioninput), [`SubscriptionOutput`](#subscriptionoutput)

- **`skipBilling(id: string, input?: SkipBillingCycleInput | undefined | null): Promise<SubscriptionOutput>`**

  Skip the next billing cycle

  Types: [`SkipBillingCycleInput`](#skipbillingcycleinput), [`SubscriptionOutput`](#subscriptionoutput)

- **`listBillingCycles(filter?: BillingCycleFilterInput | undefined | null): Promise<Array<BillingCycleOutput>>`**

  List billing cycles for a subscription

  Types: [`BillingCycleFilterInput`](#billingcyclefilterinput), [`BillingCycleOutput`](#billingcycleoutput)

- **`getBillingCycle(id: string): Promise<BillingCycleOutput | null>`**

  Get a billing cycle by ID

  Types: [`BillingCycleOutput`](#billingcycleoutput)

- **`getEvents(subscriptionId: string): Promise<Array<SubscriptionEventOutput>>`**

  Get events for a subscription

  Types: [`SubscriptionEventOutput`](#subscriptioneventoutput)

### commerce.promotions

Class `Promotions`.

Promotions API for managing discounts and coupon codes

- **`create(input: CreatePromotionInput): Promise<PromotionOutput>`**

  Create a new promotion

  Types: [`CreatePromotionInput`](#createpromotioninput), [`PromotionOutput`](#promotionoutput)

- **`get(id: string): Promise<PromotionOutput | null>`**

  Get a promotion by ID

  Types: [`PromotionOutput`](#promotionoutput)

- **`getByCode(code: string): Promise<PromotionOutput | null>`**

  Get a promotion by its internal code

  Types: [`PromotionOutput`](#promotionoutput)

- **`list(filter?: PromotionFilterInput | undefined | null): Promise<Array<PromotionOutput>>`**

  List promotions with optional filtering

  Types: [`PromotionFilterInput`](#promotionfilterinput), [`PromotionOutput`](#promotionoutput)

- **`update(id: string, input: UpdatePromotionInput): Promise<PromotionOutput>`**

  Update a promotion

  Types: [`UpdatePromotionInput`](#updatepromotioninput), [`PromotionOutput`](#promotionoutput)

- **`delete(id: string): Promise<void>`**

  Delete a promotion

- **`activate(id: string): Promise<PromotionOutput>`**

  Activate a promotion

  Types: [`PromotionOutput`](#promotionoutput)

- **`deactivate(id: string): Promise<PromotionOutput>`**

  Deactivate (pause) a promotion

  Types: [`PromotionOutput`](#promotionoutput)

- **`getActive(): Promise<Array<PromotionOutput>>`**

  Get all currently active promotions

  Types: [`PromotionOutput`](#promotionoutput)

- **`isValid(id: string): Promise<boolean>`**

  Check if a promotion is currently valid

- **`createCoupon(input: CreateCouponInput): Promise<CouponOutput>`**

  Create a coupon code for a promotion

  Types: [`CreateCouponInput`](#createcouponinput), [`CouponOutput`](#couponoutput)

- **`getCoupon(id: string): Promise<CouponOutput | null>`**

  Get a coupon by ID

  Types: [`CouponOutput`](#couponoutput)

- **`getCouponByCode(code: string): Promise<CouponOutput | null>`**

  Get a coupon by its code

  Types: [`CouponOutput`](#couponoutput)

- **`listCoupons(filter?: CouponFilterInput | undefined | null): Promise<Array<CouponOutput>>`**

  List coupons with optional filtering

  Types: [`CouponFilterInput`](#couponfilterinput), [`CouponOutput`](#couponoutput)

- **`validateCoupon(code: string): Promise<CouponOutput | null>`**

  Validate a coupon code

  Types: [`CouponOutput`](#couponoutput)

- **`apply(input: ApplyPromotionsInput): Promise<ApplyPromotionsOutput>`**

  Apply promotions to cart/order items

  Types: [`ApplyPromotionsInput`](#applypromotionsinput), [`ApplyPromotionsOutput`](#applypromotionsoutput)

- **`recordUsage(promotionId: string, couponId: string | undefined | null, customerId: string | undefined | null, orderId: string | undefined | null, cartId: string | undefined | null, discountAmount: number, currency: string): Promise<PromotionUsageOutput>`**

  Record promotion usage (after order completion)

  Types: [`PromotionUsageOutput`](#promotionusageoutput)

### commerce.tax

Class `Tax`.

- **`calculate(input: TaxCalculationInput): Promise<TaxCalculationOutput>`**

  Calculate tax for a transaction

  Types: [`TaxCalculationInput`](#taxcalculationinput), [`TaxCalculationOutput`](#taxcalculationoutput)

- **`calculateForItem(unitPrice: number, quantity: number, category: string | undefined | null, shippingAddress: TaxAddressInput): Promise<number>`**

  Calculate tax for a single item

  Types: [`TaxAddressInput`](#taxaddressinput)

- **`getEffectiveRate(address: TaxAddressInput, category?: string | undefined | null): Promise<number>`**

  Get the effective tax rate for an address and category

  Types: [`TaxAddressInput`](#taxaddressinput)

- **`getJurisdiction(id: string): Promise<TaxJurisdictionOutput | null>`**

  Get a jurisdiction by ID

  Types: [`TaxJurisdictionOutput`](#taxjurisdictionoutput)

- **`getJurisdictionByCode(code: string): Promise<TaxJurisdictionOutput | null>`**

  Get a jurisdiction by code

  Types: [`TaxJurisdictionOutput`](#taxjurisdictionoutput)

- **`listJurisdictions(filter?: JurisdictionFilterInput | undefined | null): Promise<Array<TaxJurisdictionOutput>>`**

  List jurisdictions with optional filtering

  Types: [`JurisdictionFilterInput`](#jurisdictionfilterinput), [`TaxJurisdictionOutput`](#taxjurisdictionoutput)

- **`createJurisdiction(input: CreateJurisdictionInput): Promise<TaxJurisdictionOutput>`**

  Create a new jurisdiction

  Types: [`CreateJurisdictionInput`](#createjurisdictioninput), [`TaxJurisdictionOutput`](#taxjurisdictionoutput)

- **`getRate(id: string): Promise<TaxRateOutput | null>`**

  Get a tax rate by ID

  Types: [`TaxRateOutput`](#taxrateoutput)

- **`listRates(filter?: TaxRateFilterInput | undefined | null): Promise<Array<TaxRateOutput>>`**

  List tax rates with optional filtering

  Types: [`TaxRateFilterInput`](#taxratefilterinput), [`TaxRateOutput`](#taxrateoutput)

- **`createRate(input: CreateTaxRateInput): Promise<TaxRateOutput>`**

  Create a new tax rate

  Types: [`CreateTaxRateInput`](#createtaxrateinput), [`TaxRateOutput`](#taxrateoutput)

- **`getExemption(id: string): Promise<TaxExemptionOutput | null>`**

  Get an exemption by ID

  Types: [`TaxExemptionOutput`](#taxexemptionoutput)

- **`getCustomerExemptions(customerId: string): Promise<Array<TaxExemptionOutput>>`**

  Get exemptions for a customer

  Types: [`TaxExemptionOutput`](#taxexemptionoutput)

- **`createExemption(input: CreateExemptionInput): Promise<TaxExemptionOutput>`**

  Create a tax exemption

  Types: [`CreateExemptionInput`](#createexemptioninput), [`TaxExemptionOutput`](#taxexemptionoutput)

- **`customerIsExempt(customerId: string): Promise<boolean>`**

  Check if a customer is tax exempt

- **`getSettings(): Promise<TaxSettingsOutput>`**

  Get tax settings

  Types: [`TaxSettingsOutput`](#taxsettingsoutput)

- **`updateSettings(input: TaxSettingsInput): Promise<TaxSettingsOutput>`**

  Update tax settings

  Types: [`TaxSettingsInput`](#taxsettingsinput), [`TaxSettingsOutput`](#taxsettingsoutput)

- **`setEnabled(enabled: boolean): Promise<TaxSettingsOutput>`**

  Enable or disable tax calculation

  Types: [`TaxSettingsOutput`](#taxsettingsoutput)

- **`isEnabled(): Promise<boolean>`**

  Check if tax calculation is enabled

- **`static getUsStateInfo(stateCode: string): UsStateTaxInfoOutput | null`**

  Get US state tax information

  Types: [`UsStateTaxInfoOutput`](#usstatetaxinfooutput)

- **`static getEuVatInfo(countryCode: string): EuVatInfoOutput | null`**

  Get EU VAT information

  Types: [`EuVatInfoOutput`](#euvatinfooutput)

- **`static getCanadianTaxInfo(provinceCode: string): CanadianTaxInfoOutput | null`**

  Get Canadian tax information

  Types: [`CanadianTaxInfoOutput`](#canadiantaxinfooutput)

- **`static isEuCountry(countryCode: string): boolean`**

  Check if a country is in the EU

### commerce.quality

Class `Quality`.

- **`createInspection(input: CreateInspectionInput): Promise<InspectionOutput>`**

  Create a new inspection

  Types: [`CreateInspectionInput`](#createinspectioninput), [`InspectionOutput`](#inspectionoutput)

- **`getInspection(id: string): Promise<InspectionOutput | null>`**

  Get an inspection by ID

  Types: [`InspectionOutput`](#inspectionoutput)

- **`listInspections(filter?: InspectionFilterInput | undefined | null): Promise<Array<InspectionOutput>>`**

  List inspections, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (server default page size).

  Types: [`InspectionFilterInput`](#inspectionfilterinput), [`InspectionOutput`](#inspectionoutput)

- **`startInspection(id: string): Promise<InspectionOutput>`**

  Start an inspection

  Types: [`InspectionOutput`](#inspectionoutput)

- **`completeInspection(id: string): Promise<InspectionOutput>`**

  Complete an inspection

  Types: [`InspectionOutput`](#inspectionoutput)

- **`createNcr(input: CreateNcrInput): Promise<NcrOutput>`**

  Create a non-conformance report

  Types: [`CreateNcrInput`](#createncrinput), [`NcrOutput`](#ncroutput)

- **`getNcr(id: string): Promise<NcrOutput | null>`**

  Get an NCR by ID

  Types: [`NcrOutput`](#ncroutput)

- **`listNcrs(filter?: NcrFilterInput | undefined | null): Promise<Array<NcrOutput>>`**

  List NCRs, optionally filtered/paginated.

  Calling with no argument keeps the previous behaviour (server default page size).

  Types: [`NcrFilterInput`](#ncrfilterinput), [`NcrOutput`](#ncroutput)

- **`closeNcr(id: string): Promise<NcrOutput>`**

  Close an NCR

  Types: [`NcrOutput`](#ncroutput)

- **`createHold(input: CreateQualityHoldInput): Promise<QualityHoldOutput>`**

  Create a quality hold

  Types: [`CreateQualityHoldInput`](#createqualityholdinput), [`QualityHoldOutput`](#qualityholdoutput)

- **`getHold(id: string): Promise<QualityHoldOutput | null>`**

  Get a quality hold by ID

  Types: [`QualityHoldOutput`](#qualityholdoutput)

- **`listHolds(filter?: QualityHoldFilterInput | undefined | null): Promise<Array<QualityHoldOutput>>`**

  List quality holds, optionally filtered and paginated. No argument lists all.

  Types: [`QualityHoldFilterInput`](#qualityholdfilterinput), [`QualityHoldOutput`](#qualityholdoutput)

- **`releaseHold(id: string, releasedBy: string, notes?: string | undefined | null): Promise<QualityHoldOutput>`**

  Release a quality hold

  Types: [`QualityHoldOutput`](#qualityholdoutput)

- **`getActiveHolds(): Promise<Array<QualityHoldOutput>>`**

  Get all active holds

  Types: [`QualityHoldOutput`](#qualityholdoutput)

- **`countActiveHolds(): Promise<number>`**

  Count active holds

### commerce.lots

Class `Lots`.

- **`create(input: CreateLotInput): Promise<LotOutput>`**

  Create a new lot

  Types: [`CreateLotInput`](#createlotinput), [`LotOutput`](#lotoutput)

- **`get(id: string): Promise<LotOutput | null>`**

  Get a lot by ID

  Types: [`LotOutput`](#lotoutput)

- **`getByNumber(lotNumber: string): Promise<LotOutput | null>`**

  Get a lot by lot number

  Types: [`LotOutput`](#lotoutput)

- **`list(filter?: LotFilterInput | undefined | null): Promise<Array<LotOutput>>`**

  List lots, optionally filtered and paginated. No argument lists all.

  Types: [`LotFilterInput`](#lotfilterinput), [`LotOutput`](#lotoutput)

- **`getActiveLots(sku: string): Promise<Array<LotOutput>>`**

  Get active lots for a SKU

  Types: [`LotOutput`](#lotoutput)

- **`getAvailableLotsForSku(sku: string): Promise<Array<LotOutput>>`**

  Get available lots for a SKU (FIFO order)

  Types: [`LotOutput`](#lotoutput)

- **`quarantine(id: string, reason: string): Promise<LotOutput>`**

  Quarantine a lot

  Types: [`LotOutput`](#lotoutput)

- **`releaseQuarantine(id: string): Promise<LotOutput>`**

  Release a lot from quarantine

  Types: [`LotOutput`](#lotoutput)

- **`getExpiringLots(days: number): Promise<Array<LotOutput>>`**

  Get expiring lots within days

  Types: [`LotOutput`](#lotoutput)

- **`getExpiredLots(): Promise<Array<LotOutput>>`**

  Get expired lots

  Types: [`LotOutput`](#lotoutput)

- **`getQuarantined(): Promise<Array<LotOutput>>`**

  Get quarantined lots

  Types: [`LotOutput`](#lotoutput)

- **`count(): Promise<number>`**

  Count lots

### commerce.serials

Class `Serials`.

- **`create(input: CreateSerialInput): Promise<SerialOutput>`**

  Create a serial number

  Types: [`CreateSerialInput`](#createserialinput), [`SerialOutput`](#serialoutput)

- **`get(id: string): Promise<SerialOutput | null>`**

  Get a serial by ID

  Types: [`SerialOutput`](#serialoutput)

- **`getBySerial(serial: string): Promise<SerialOutput | null>`**

  Get a serial by serial number string

  Types: [`SerialOutput`](#serialoutput)

- **`list(filter?: SerialFilterInput | undefined | null): Promise<Array<SerialOutput>>`**

  List serials, optionally filtered and paginated. No argument lists all.

  Types: [`SerialFilterInput`](#serialfilterinput), [`SerialOutput`](#serialoutput)

- **`getAvailable(sku: string, limit: number): Promise<Array<SerialOutput>>`**

  Get available serials for a SKU

  Types: [`SerialOutput`](#serialoutput)

- **`markSold(id: string, customerId: string, orderId?: string | undefined | null): Promise<SerialOutput>`**

  Mark a serial as sold

  Types: [`SerialOutput`](#serialoutput)

- **`quarantine(id: string, reason: string): Promise<SerialOutput>`**

  Quarantine a serial

  Types: [`SerialOutput`](#serialoutput)

- **`isAvailable(serial: string): Promise<boolean>`**

  Check if a serial is available

- **`count(): Promise<number>`**

  Count serials

### commerce.warehouse

Class `Warehouse`.

- **`createWarehouse(input: CreateWarehouseInput): Promise<WarehouseOutput>`**

  Create a new warehouse

  Types: [`CreateWarehouseInput`](#createwarehouseinput), [`WarehouseOutput`](#warehouseoutput)

- **`getWarehouse(id: number): Promise<WarehouseOutput | null>`**

  Get a warehouse by ID

  Types: [`WarehouseOutput`](#warehouseoutput)

- **`getWarehouseByCode(code: string): Promise<WarehouseOutput | null>`**

  Get a warehouse by code

  Types: [`WarehouseOutput`](#warehouseoutput)

- **`listWarehouses(filter?: WarehouseFilterInput | undefined | null): Promise<Array<WarehouseOutput>>`**

  List warehouses, optionally filtered and paginated. No argument lists all.

  Types: [`WarehouseFilterInput`](#warehousefilterinput), [`WarehouseOutput`](#warehouseoutput)

- **`createLocation(input: CreateLocationInput): Promise<LocationOutput>`**

  Create a new location

  Types: [`CreateLocationInput`](#createlocationinput), [`LocationOutput`](#locationoutput)

- **`getLocation(id: number): Promise<LocationOutput | null>`**

  Get a location by ID

  Types: [`LocationOutput`](#locationoutput)

- **`listLocations(warehouseId?: number | undefined | null): Promise<Array<LocationOutput>>`**

  List locations in a warehouse

  Types: [`LocationOutput`](#locationoutput)

- **`getPickableLocations(warehouseId: number, sku: string): Promise<Array<LocationOutput>>`**

  Get pickable locations for a SKU

  Types: [`LocationOutput`](#locationoutput)

- **`getTotalAvailable(warehouseId: number, sku: string): Promise<number>`**

  Get total available quantity for a SKU in a warehouse

- **`countWarehouses(): Promise<number>`**

  Count warehouses

### commerce.receiving

Class `Receiving`.

- **`createReceipt(input: CreateReceiptInput): Promise<ReceiptOutput>`**

  Create a new receipt

  Types: [`CreateReceiptInput`](#createreceiptinput), [`ReceiptOutput`](#receiptoutput)

- **`getReceipt(id: string): Promise<ReceiptOutput | null>`**

  Get a receipt by ID

  Types: [`ReceiptOutput`](#receiptoutput)

- **`getReceiptByNumber(number: string): Promise<ReceiptOutput | null>`**

  Get a receipt by receipt number

  Types: [`ReceiptOutput`](#receiptoutput)

- **`listReceipts(filter?: ReceiptFilterInput | undefined | null): Promise<Array<ReceiptOutput>>`**

  List receipts, optionally filtered and paginated. No argument lists all.

  Types: [`ReceiptFilterInput`](#receiptfilterinput), [`ReceiptOutput`](#receiptoutput)

- **`startReceiving(id: string): Promise<ReceiptOutput>`**

  Start receiving

  Types: [`ReceiptOutput`](#receiptoutput)

- **`completeReceiving(id: string): Promise<ReceiptOutput>`**

  Complete receiving

  Types: [`ReceiptOutput`](#receiptoutput)

- **`cancelReceipt(id: string): Promise<ReceiptOutput>`**

  Cancel a receipt

  Types: [`ReceiptOutput`](#receiptoutput)

- **`createReceiptFromPo(poId: string, warehouseId: number): Promise<ReceiptOutput>`**

  Create a receipt from a purchase order

  Types: [`ReceiptOutput`](#receiptoutput)

- **`countReceipts(): Promise<number>`**

  Count receipts

### commerce.fulfillment

Class `Fulfillment`.

- **`createWave(input: CreateWaveInput): Promise<WaveOutput>`**

  Create a wave

  Types: [`CreateWaveInput`](#createwaveinput), [`WaveOutput`](#waveoutput)

- **`getWave(id: string): Promise<WaveOutput | null>`**

  Get a wave by ID

  Types: [`WaveOutput`](#waveoutput)

- **`listWaves(filter?: WaveFilterInput | undefined | null): Promise<Array<WaveOutput>>`**

  List waves, optionally filtered and paginated. No argument lists all.

  Types: [`WaveFilterInput`](#wavefilterinput), [`WaveOutput`](#waveoutput)

- **`releaseWave(id: string): Promise<WaveOutput>`**

  Release a wave for picking

  Types: [`WaveOutput`](#waveoutput)

- **`completeWave(id: string): Promise<WaveOutput>`**

  Complete a wave

  Types: [`WaveOutput`](#waveoutput)

- **`cancelWave(id: string): Promise<WaveOutput>`**

  Cancel a wave

  Types: [`WaveOutput`](#waveoutput)

- **`getPick(id: string): Promise<PickTaskOutput | null>`**

  Get a pick task by ID

  Types: [`PickTaskOutput`](#picktaskoutput)

- **`listPicks(filter?: PickTaskFilterInput | undefined | null): Promise<Array<PickTaskOutput>>`**

  List pick tasks, optionally filtered and paginated. No argument lists all.

  Types: [`PickTaskFilterInput`](#picktaskfilterinput), [`PickTaskOutput`](#picktaskoutput)

- **`assignPick(id: string, assignedTo: string): Promise<PickTaskOutput>`**

  Assign a pick task

  Types: [`PickTaskOutput`](#picktaskoutput)

- **`startPick(id: string): Promise<PickTaskOutput>`**

  Start a pick task

  Types: [`PickTaskOutput`](#picktaskoutput)

- **`cancelPick(id: string): Promise<PickTaskOutput>`**

  Cancel a pick task

  Types: [`PickTaskOutput`](#picktaskoutput)

- **`isOrderReadyToPack(orderId: string): Promise<boolean>`**

  Check if an order is ready to pack

- **`isOrderReadyToShip(orderId: string): Promise<boolean>`**

  Check if an order is ready to ship

- **`countWaves(): Promise<number>`**

  Count waves

### commerce.accountsPayable

Class `AccountsPayable`.

- **`createBill(input: CreateBillInput): Promise<BillOutput>`**

  Create a bill

  Types: [`CreateBillInput`](#createbillinput), [`BillOutput`](#billoutput)

- **`getBill(id: string): Promise<BillOutput | null>`**

  Get a bill by ID

  Types: [`BillOutput`](#billoutput)

- **`getBillByNumber(number: string): Promise<BillOutput | null>`**

  Get a bill by bill number

  Types: [`BillOutput`](#billoutput)

- **`listBills(filter?: BillFilterInput | undefined | null): Promise<Array<BillOutput>>`**

  List bills, optionally filtered and paginated. No argument lists all.

  Types: [`BillFilterInput`](#billfilterinput), [`BillOutput`](#billoutput)

- **`approveBill(id: string): Promise<BillOutput>`**

  Approve a bill

  Types: [`BillOutput`](#billoutput)

- **`cancelBill(id: string): Promise<BillOutput>`**

  Cancel a bill

  Types: [`BillOutput`](#billoutput)

- **`getOverdueBills(): Promise<Array<BillOutput>>`**

  Get overdue bills

  Types: [`BillOutput`](#billoutput)

- **`getBillsDueSoon(days: number): Promise<Array<BillOutput>>`**

  Get bills due soon

  Types: [`BillOutput`](#billoutput)

- **`getAgingSummary(): Promise<ApAgingSummaryOutput>`**

  Get aging summary

  Types: [`ApAgingSummaryOutput`](#apagingsummaryoutput)

- **`getTotalOutstanding(): Promise<number>`**

  Get total outstanding

- **`countBills(): Promise<number>`**

  Count bills

- **`threeWayMatch(billId: string, tolerancePercent?: string | undefined | null): Promise<ThreeWayMatchOutput>`**

  Three-way match a bill against its purchase order and receipts.

  `tolerance_percent` is an exact decimal string (e.g. "5" for 5%);
  omit it for exact matching.

  Types: [`ThreeWayMatchOutput`](#threewaymatchoutput)

### commerce.accountsReceivable

Class `AccountsReceivable`.

- **`getAgingSummary(): Promise<ArAgingSummaryOutput>`**

  Get AR aging summary

  Types: [`ArAgingSummaryOutput`](#aragingsummaryoutput)

- **`getTotalOutstanding(): Promise<number>`**

  Get total outstanding

- **`getDso(days: number): Promise<number>`**

  Get Days Sales Outstanding (DSO)

- **`createCreditMemo(input: CreateCreditMemoInput): Promise<CreditMemoOutput>`**

  Create a credit memo

  Types: [`CreateCreditMemoInput`](#createcreditmemoinput), [`CreditMemoOutput`](#creditmemooutput)

- **`getCreditMemo(id: string): Promise<CreditMemoOutput | null>`**

  Get a credit memo by ID

  Types: [`CreditMemoOutput`](#creditmemooutput)

- **`listCreditMemos(filter?: CreditMemoFilterInput | undefined | null): Promise<Array<CreditMemoOutput>>`**

  List credit memos, optionally filtered and paginated. No argument lists all.

  Types: [`CreditMemoFilterInput`](#creditmemofilterinput), [`CreditMemoOutput`](#creditmemooutput)

- **`voidCreditMemo(id: string): Promise<CreditMemoOutput>`**

  Void a credit memo

  Types: [`CreditMemoOutput`](#creditmemooutput)

- **`getUnappliedCredits(customerId: string): Promise<Array<CreditMemoOutput>>`**

  Get unapplied credits for a customer

  Types: [`CreditMemoOutput`](#creditmemooutput)

### commerce.costAccounting

Class `CostAccounting`.

- **`getItemCost(sku: string): Promise<ItemCostOutput | null>`**

  Get item cost

  Types: [`ItemCostOutput`](#itemcostoutput)

- **`setItemCost(input: SetItemCostInput): Promise<ItemCostOutput>`**

  Set item cost

  Types: [`SetItemCostInput`](#setitemcostinput), [`ItemCostOutput`](#itemcostoutput)

- **`listItemCosts(filter?: ItemCostFilterInput | undefined | null): Promise<Array<ItemCostOutput>>`**

  List item costs, optionally filtered and paginated. No argument lists all.

  Types: [`ItemCostFilterInput`](#itemcostfilterinput), [`ItemCostOutput`](#itemcostoutput)

- **`updateAverageCost(sku: string, quantity: number, unitCost: number): Promise<ItemCostOutput>`**

  Update average cost

  Types: [`ItemCostOutput`](#itemcostoutput)

- **`getTotalInventoryValue(): Promise<number>`**

  Get total inventory value

### commerce.credit

Class `Credit`.

- **`createCreditAccount(input: CreateCreditAccountInput): Promise<CreditAccountOutput>`**

  Create a credit account

  Types: [`CreateCreditAccountInput`](#createcreditaccountinput), [`CreditAccountOutput`](#creditaccountoutput)

- **`getCreditAccount(id: string): Promise<CreditAccountOutput | null>`**

  Get a credit account by ID

  Types: [`CreditAccountOutput`](#creditaccountoutput)

- **`getCreditAccountByCustomer(customerId: string): Promise<CreditAccountOutput | null>`**

  Get credit account by customer

  Types: [`CreditAccountOutput`](#creditaccountoutput)

- **`listCreditAccounts(filter?: CreditAccountFilterInput | undefined | null): Promise<Array<CreditAccountOutput>>`**

  List credit accounts, optionally filtered and paginated. No argument lists all.

  Types: [`CreditAccountFilterInput`](#creditaccountfilterinput), [`CreditAccountOutput`](#creditaccountoutput)

- **`checkCredit(customerId: string, orderAmount: number): Promise<CreditCheckOutput>`**

  Check credit

  Types: [`CreditCheckOutput`](#creditcheckoutput)

- **`adjustCreditLimit(customerId: string, newLimit: number, reason: string): Promise<CreditAccountOutput>`**

  Adjust credit limit

  Types: [`CreditAccountOutput`](#creditaccountoutput)

- **`suspendCreditAccount(customerId: string, reason: string): Promise<CreditAccountOutput>`**

  Suspend credit account

  Types: [`CreditAccountOutput`](#creditaccountoutput)

- **`reactivateCreditAccount(customerId: string): Promise<CreditAccountOutput>`**

  Reactivate credit account

  Types: [`CreditAccountOutput`](#creditaccountoutput)

- **`getOverLimitCustomers(): Promise<Array<CreditAccountOutput>>`**

  Get over-limit customers

  Types: [`CreditAccountOutput`](#creditaccountoutput)

### commerce.backorder

Class `Backorders`. Also available as `commerce.backorders`.

- **`createBackorder(input: CreateBackorderInput): Promise<BackorderOutput>`**

  Create a backorder

  Types: [`CreateBackorderInput`](#createbackorderinput), [`BackorderOutput`](#backorderoutput)

- **`getBackorder(id: string): Promise<BackorderOutput | null>`**

  Get a backorder by ID

  Types: [`BackorderOutput`](#backorderoutput)

- **`getBackorderByNumber(number: string): Promise<BackorderOutput | null>`**

  Get a backorder by number

  Types: [`BackorderOutput`](#backorderoutput)

- **`listBackorders(filter?: BackorderFilterInput | undefined | null): Promise<Array<BackorderOutput>>`**

  List backorders, optionally filtered and paginated. No argument lists all.

  Types: [`BackorderFilterInput`](#backorderfilterinput), [`BackorderOutput`](#backorderoutput)

- **`cancelBackorder(id: string): Promise<BackorderOutput>`**

  Cancel a backorder

  Types: [`BackorderOutput`](#backorderoutput)

- **`getBackordersForOrder(orderId: string): Promise<Array<BackorderOutput>>`**

  Get backorders for an order

  Types: [`BackorderOutput`](#backorderoutput)

- **`getBackordersForSku(sku: string): Promise<Array<BackorderOutput>>`**

  Get backorders for a SKU

  Types: [`BackorderOutput`](#backorderoutput)

- **`getOverdueBackorders(): Promise<Array<BackorderOutput>>`**

  Get overdue backorders

  Types: [`BackorderOutput`](#backorderoutput)

- **`getSummary(): Promise<BackorderSummaryOutput>`**

  Get backorder summary

  Types: [`BackorderSummaryOutput`](#backordersummaryoutput)

- **`countPending(): Promise<number>`**

  Count pending backorders

### commerce.generalLedger

Class `GeneralLedger`.

- **`createAccount(input: CreateGlAccountInput): Promise<GlAccountOutput>`**

  Create a GL account

  Types: [`CreateGlAccountInput`](#createglaccountinput), [`GlAccountOutput`](#glaccountoutput)

- **`getAccount(id: string): Promise<GlAccountOutput | null>`**

  Get a GL account by ID

  Types: [`GlAccountOutput`](#glaccountoutput)

- **`getAccountByNumber(accountNumber: string): Promise<GlAccountOutput | null>`**

  Get a GL account by account number

  Types: [`GlAccountOutput`](#glaccountoutput)

- **`listAccounts(filter?: GlAccountFilterInput | undefined | null): Promise<Array<GlAccountOutput>>`**

  List GL accounts, optionally filtered and paginated. No argument lists all.

  Types: [`GlAccountFilterInput`](#glaccountfilterinput), [`GlAccountOutput`](#glaccountoutput)

- **`initializeChartOfAccounts(): Promise<Array<GlAccountOutput>>`**

  Initialize standard chart of accounts

  Types: [`GlAccountOutput`](#glaccountoutput)

- **`getJournalEntry(id: string): Promise<JournalEntryOutput | null>`**

  Get a journal entry by ID

  Types: [`JournalEntryOutput`](#journalentryoutput)

- **`listJournalEntries(filter?: JournalEntryFilterInput | undefined | null): Promise<Array<JournalEntryOutput>>`**

  List journal entries, optionally filtered and paginated. No argument lists all.

  Types: [`JournalEntryFilterInput`](#journalentryfilterinput), [`JournalEntryOutput`](#journalentryoutput)

- **`postJournalEntry(id: string, postedBy: string): Promise<JournalEntryOutput>`**

  Post a journal entry

  Types: [`JournalEntryOutput`](#journalentryoutput)

- **`voidJournalEntry(id: string): Promise<JournalEntryOutput>`**

  Void a journal entry

  Types: [`JournalEntryOutput`](#journalentryoutput)

- **`getTrialBalance(asOfDate: string): Promise<TrialBalanceOutput>`**

  Get trial balance

  Types: [`TrialBalanceOutput`](#trialbalanceoutput)

- **`getBalanceSheet(asOfDate: string): Promise<BalanceSheetOutput>`**

  Get balance sheet

  Types: [`BalanceSheetOutput`](#balancesheetoutput)

- **`getIncomeStatement(startDate: string, endDate: string): Promise<IncomeStatementOutput>`**

  Get income statement

  Types: [`IncomeStatementOutput`](#incomestatementoutput)

- **`getAccountBalance(accountId: string, asOfDate?: string | undefined | null): Promise<number>`**

  Get account balance

- **`revalue(asOfDate: string, baseCurrency?: string | undefined | null): Promise<RevaluationOutput>`**

  Revalue foreign-currency account balances at the as-of exchange rate.

  `as_of_date` is an ISO date (YYYY-MM-DD); `base_currency` defaults to
  the store's configured base currency.

  Types: [`RevaluationOutput`](#revaluationoutput)

- **`createPeriod(input: CreateGlPeriodInput): Promise<GlPeriodOutput>`**

  Create an accounting period.

  Types: [`CreateGlPeriodInput`](#createglperiodinput), [`GlPeriodOutput`](#glperiodoutput)

- **`openPeriod(id: string): Promise<GlPeriodOutput>`**

  Open a period (transition from future to open).

  Types: [`GlPeriodOutput`](#glperiodoutput)

- **`listPeriods(filter?: GlPeriodFilterInput | undefined | null): Promise<Array<GlPeriodOutput>>`**

  List accounting periods with optional filtering.

  Types: [`GlPeriodFilterInput`](#glperiodfilterinput), [`GlPeriodOutput`](#glperiodoutput)

- **`closeMonth(periodId: string, options?: CloseMonthOptionsInput | undefined | null): Promise<CloseMonthReportOutput>`**

  Close the month: post scheduled depreciation, recognize revenue
  through period end, revalue foreign-currency balances, then run the
  period close (closing entries + close period).

  Pass `{ dryRun: true }` to compute per-step counts and amounts without
  writing anything.

  Types: [`CloseMonthOptionsInput`](#closemonthoptionsinput), [`CloseMonthReportOutput`](#closemonthreportoutput)

### commerce.fixedAssets

Class `FixedAssets`.

- **`isSupported(): Promise<boolean>`**

  Whether the fixed-assets backend is available on this engine build.

- **`create(input: CreateFixedAssetInput): Promise<FixedAssetOutput>`**

  Types: [`CreateFixedAssetInput`](#createfixedassetinput), [`FixedAssetOutput`](#fixedassetoutput)

- **`get(id: string): Promise<FixedAssetOutput | null>`**

  Types: [`FixedAssetOutput`](#fixedassetoutput)

- **`list(filter?: FixedAssetFilterInput | undefined | null): Promise<Array<FixedAssetOutput>>`**

  Types: [`FixedAssetFilterInput`](#fixedassetfilterinput), [`FixedAssetOutput`](#fixedassetoutput)

- **`update(id: string, input: UpdateFixedAssetInput): Promise<FixedAssetOutput>`**

  Types: [`UpdateFixedAssetInput`](#updatefixedassetinput), [`FixedAssetOutput`](#fixedassetoutput)

- **`placeInService(id: string, date: string): Promise<FixedAssetOutput>`**

  Place a draft asset in service on the given ISO date (YYYY-MM-DD).

  Types: [`FixedAssetOutput`](#fixedassetoutput)

- **`dispose(id: string, proceeds: string, date?: string | undefined | null, notes?: string | undefined | null): Promise<FixedAssetOutput>`**

  Dispose of an asset for the given proceeds (exact decimal string),
  recording gain/loss. `date` is an ISO date (YYYY-MM-DD); defaults to today.

  Types: [`FixedAssetOutput`](#fixedassetoutput)

- **`writeOff(id: string, date?: string | undefined | null, notes?: string | undefined | null): Promise<FixedAssetOutput>`**

  Write off an asset (disposal with zero proceeds). `date` is an ISO date
  (YYYY-MM-DD); defaults to today.

  Types: [`FixedAssetOutput`](#fixedassetoutput)

- **`generateSchedule(id: string): Promise<DepreciationScheduleOutput>`**

  Generate and persist the depreciation schedule for an asset.

  Types: [`DepreciationScheduleOutput`](#depreciationscheduleoutput)

- **`getSchedule(id: string): Promise<DepreciationScheduleOutput | null>`**

  Get the persisted depreciation schedule for an asset, if generated.

  Types: [`DepreciationScheduleOutput`](#depreciationscheduleoutput)

- **`postDepreciation(id: string, periods: number): Promise<FixedAssetOutput>`**

  Post the next `periods` scheduled depreciation entries.

  Types: [`FixedAssetOutput`](#fixedassetoutput)

### commerce.revenueRecognition

Class `RevenueRecognition`.

- **`isSupported(): Promise<boolean>`**

  Whether the revenue-recognition backend is available on this engine build.

- **`createContract(input: CreateRevenueContractInput): Promise<RevenueContractOutput>`**

  Types: [`CreateRevenueContractInput`](#createrevenuecontractinput), [`RevenueContractOutput`](#revenuecontractoutput)

- **`getContract(id: string): Promise<RevenueContractOutput | null>`**

  Types: [`RevenueContractOutput`](#revenuecontractoutput)

- **`listContracts(filter?: RevenueContractFilterInput | undefined | null): Promise<Array<RevenueContractOutput>>`**

  Types: [`RevenueContractFilterInput`](#revenuecontractfilterinput), [`RevenueContractOutput`](#revenuecontractoutput)

- **`updateContract(id: string, input: UpdateRevenueContractInput): Promise<RevenueContractOutput>`**

  Types: [`UpdateRevenueContractInput`](#updaterevenuecontractinput), [`RevenueContractOutput`](#revenuecontractoutput)

- **`listObligations(contractId: string): Promise<Array<PerformanceObligationOutput>>`**

  List the performance obligations under a contract.

  Types: [`PerformanceObligationOutput`](#performanceobligationoutput)

- **`generateSchedule(obligationId: string): Promise<RevenueScheduleOutput>`**

  Generate and persist the recognition schedule for an obligation.

  Types: [`RevenueScheduleOutput`](#revenuescheduleoutput)

- **`getSchedule(obligationId: string): Promise<RevenueScheduleOutput | null>`**

  Get the persisted recognition schedule for an obligation, if generated.

  Types: [`RevenueScheduleOutput`](#revenuescheduleoutput)

- **`recognize(obligationId: string, through: string): Promise<RevenueScheduleOutput>`**

  Recognize deferred entries with a period start on or before `through`
  (ISO date, YYYY-MM-DD).

  Types: [`RevenueScheduleOutput`](#revenuescheduleoutput)

### commerce.cycleCounts

Class `CycleCounts`.

- **`create(input: CreateCycleCountInput): Promise<CycleCountOutput>`**

  Create a cycle count (draft) with its expected lines.

  Types: [`CreateCycleCountInput`](#createcyclecountinput), [`CycleCountOutput`](#cyclecountoutput)

- **`get(id: string): Promise<CycleCountOutput | null>`**

  Get a cycle count (with lines) by ID.

  Types: [`CycleCountOutput`](#cyclecountoutput)

- **`list(filter?: CycleCountFilterInput | undefined | null): Promise<Array<CycleCountOutput>>`**

  List cycle counts matching the filter.

  Types: [`CycleCountFilterInput`](#cyclecountfilterinput), [`CycleCountOutput`](#cyclecountoutput)

- **`start(id: string): Promise<CycleCountOutput>`**

  Start a draft cycle count (draft -> in_progress).

  Types: [`CycleCountOutput`](#cyclecountoutput)

- **`recordCounts(id: string, counts: Array<RecordCycleCountLineInput>): Promise<CycleCountOutput>`**

  Record physical counts against an in-progress cycle count.

  Types: [`RecordCycleCountLineInput`](#recordcyclecountlineinput), [`CycleCountOutput`](#cyclecountoutput)

- **`complete(id: string): Promise<CycleCountOutput>`**

  Complete an in-progress cycle count, applying variance adjustments.

  Types: [`CycleCountOutput`](#cyclecountoutput)

- **`cancel(id: string): Promise<CycleCountOutput>`**

  Cancel a draft or in-progress cycle count. No adjustments are applied.

  Types: [`CycleCountOutput`](#cyclecountoutput)

### commerce.ediDocuments

Class `EdiDocuments`.

- **`create(input: CreateEdiDocumentInput): Promise<EdiDocumentOutput>`**

  Create / ingest an EDI document.

  Types: [`CreateEdiDocumentInput`](#createedidocumentinput), [`EdiDocumentOutput`](#edidocumentoutput)

- **`get(id: string): Promise<EdiDocumentOutput | null>`**

  Get an EDI document by ID.

  Types: [`EdiDocumentOutput`](#edidocumentoutput)

- **`list(filter?: EdiDocumentFilterInput | undefined | null): Promise<Array<EdiDocumentOutput>>`**

  List EDI documents with optional filtering.

  Types: [`EdiDocumentFilterInput`](#edidocumentfilterinput), [`EdiDocumentOutput`](#edidocumentoutput)

- **`setStatus(id: string, status: EdiStatus, errorMessage?: string | undefined | null): Promise<EdiDocumentOutput>`**

  Update an EDI document's status.

  `status` is one of `pending`, `sent`, `acknowledged`, `processed`, `error`;
  `error_message` records failure detail when the status is `error`.

  Types: [`EdiStatus`](#edistatus), [`EdiDocumentOutput`](#edidocumentoutput)

- **`summary(): Promise<EdiSummaryOutput>`**

  Aggregate summary across all EDI documents (counts by status and type).

  Types: [`EdiSummaryOutput`](#edisummaryoutput)

### commerce.activityLogs

Class `ActivityLogs`.

- **`isSupported(): Promise<boolean>`**

  Whether the activity-logs backend is available on this engine build.

- **`record(input: RecordActivityInput): Promise<ActivityLogEntryOutput>`**

  Record an activity log entry.

  Types: [`RecordActivityInput`](#recordactivityinput), [`ActivityLogEntryOutput`](#activitylogentryoutput)

- **`get(id: string): Promise<ActivityLogEntryOutput | null>`**

  Types: [`ActivityLogEntryOutput`](#activitylogentryoutput)

- **`list(filter?: ActivityLogFilterInput | undefined | null): Promise<Array<ActivityLogEntryOutput>>`**

  Types: [`ActivityLogFilterInput`](#activitylogfilterinput), [`ActivityLogEntryOutput`](#activitylogentryoutput)

- **`historyForSubject(subjectType: string, subjectId: string): Promise<Array<ActivityLogEntryOutput>>`**

  Full history for a single subject, most recent first.

  Types: [`ActivityLogEntryOutput`](#activitylogentryoutput)

### commerce.channels

Class `Channels`.

- **`isSupported(): Promise<boolean>`**

  Whether the channels backend is available on this engine build.

- **`create(input: CreateChannelInput): Promise<ChannelOutput>`**

  Types: [`CreateChannelInput`](#createchannelinput), [`ChannelOutput`](#channeloutput)

- **`get(id: string): Promise<ChannelOutput | null>`**

  Types: [`ChannelOutput`](#channeloutput)

- **`update(id: string, input: UpdateChannelInput): Promise<ChannelOutput>`**

  Types: [`UpdateChannelInput`](#updatechannelinput), [`ChannelOutput`](#channeloutput)

- **`list(filter?: ChannelFilterInput | undefined | null): Promise<Array<ChannelOutput>>`**

  Types: [`ChannelFilterInput`](#channelfilterinput), [`ChannelOutput`](#channeloutput)

- **`delete(id: string): Promise<void>`**

  Soft-delete a channel.

- **`setLock(id: string, locked: boolean): Promise<ChannelOutput>`**

  Lock or unlock a channel against external mutations.

  Types: [`ChannelOutput`](#channeloutput)

- **`syncProducts(id: string, items: Array<ChannelProductSyncItemInput>): Promise<number>`**

  Bulk upsert/delete channel SKU mappings. Returns the affected count.

  Types: [`ChannelProductSyncItemInput`](#channelproductsynciteminput)

- **`listProductMappings(id: string): Promise<Array<ChannelProductMappingOutput>>`**

  List a channel's SKU mappings.

  Types: [`ChannelProductMappingOutput`](#channelproductmappingoutput)

### commerce.companies

Class `Companies`.

- **`isSupported(): Promise<boolean>`**

  Whether the companies backend is available on this engine build.

- **`create(input: CreateCompanyInput): Promise<CompanyOutput>`**

  Types: [`CreateCompanyInput`](#createcompanyinput), [`CompanyOutput`](#companyoutput)

- **`get(id: string): Promise<CompanyOutput | null>`**

  Types: [`CompanyOutput`](#companyoutput)

- **`update(id: string, input: UpdateCompanyInput): Promise<CompanyOutput>`**

  Types: [`UpdateCompanyInput`](#updatecompanyinput), [`CompanyOutput`](#companyoutput)

- **`list(filter?: CompanyFilterInput | undefined | null): Promise<Array<CompanyOutput>>`**

  Types: [`CompanyFilterInput`](#companyfilterinput), [`CompanyOutput`](#companyoutput)

- **`delete(id: string): Promise<void>`**

- **`listAddresses(id: string): Promise<Array<CompanyShippingAddressOutput>>`**

  List a company's shipping addresses.

  Types: [`CompanyShippingAddressOutput`](#companyshippingaddressoutput)

- **`listPriceOverrides(id: string): Promise<Array<CompanyPriceOverrideOutput>>`**

  List a company's product price overrides.

  Types: [`CompanyPriceOverrideOutput`](#companypriceoverrideoutput)

- **`createContact(input: CreateContactInput): Promise<ContactOutput>`**

  Create a contact linked to one or more companies.

  Types: [`CreateContactInput`](#createcontactinput), [`ContactOutput`](#contactoutput)

- **`getContact(id: string): Promise<ContactOutput | null>`**

  Types: [`ContactOutput`](#contactoutput)

- **`listContacts(companyId: string): Promise<Array<ContactOutput>>`**

  List contacts for a company.

  Types: [`ContactOutput`](#contactoutput)

### commerce.unitsOfMeasure

Class `UnitsOfMeasure`.

- **`isSupported(): Promise<boolean>`**

  Whether the units-of-measure backend is available on this engine build.

- **`createClass(input: CreateUnitClassInput): Promise<UnitClassOutput>`**

  Types: [`CreateUnitClassInput`](#createunitclassinput), [`UnitClassOutput`](#unitclassoutput)

- **`listClasses(filter?: UnitClassFilterInput | undefined | null): Promise<Array<UnitClassOutput>>`**

  List unit classes, optionally windowed by `limit`/`offset`.

  Types: [`UnitClassFilterInput`](#unitclassfilterinput), [`UnitClassOutput`](#unitclassoutput)

- **`deleteClass(id: string): Promise<void>`**

- **`createUom(input: CreateUnitOfMeasureInput): Promise<UnitOfMeasureOutput>`**

  Types: [`CreateUnitOfMeasureInput`](#createunitofmeasureinput), [`UnitOfMeasureOutput`](#unitofmeasureoutput)

- **`listUoms(filter?: UnitOfMeasureFilterInput | undefined | null): Promise<Array<UnitOfMeasureOutput>>`**

  Types: [`UnitOfMeasureFilterInput`](#unitofmeasurefilterinput), [`UnitOfMeasureOutput`](#unitofmeasureoutput)

- **`setBaseUom(id: string): Promise<UnitOfMeasureOutput>`**

  Mark a UOM as the base unit for its class.

  Types: [`UnitOfMeasureOutput`](#unitofmeasureoutput)

- **`deleteUom(id: string): Promise<void>`**

- **`createRule(input: CreateUnitConversionRuleInput): Promise<UnitConversionRuleOutput>`**

  Types: [`CreateUnitConversionRuleInput`](#createunitconversionruleinput), [`UnitConversionRuleOutput`](#unitconversionruleoutput)

- **`listRules(filter?: UnitConversionRuleFilterInput | undefined | null): Promise<Array<UnitConversionRuleOutput>>`**

  List conversion rules, optionally filtered by scope / product and
  windowed by `limit`/`offset`.

  Types: [`UnitConversionRuleFilterInput`](#unitconversionrulefilterinput), [`UnitConversionRuleOutput`](#unitconversionruleoutput)

- **`deleteRule(id: string): Promise<void>`**

### commerce.shippingZones

Class `ShippingZones`.

- **`isSupported(): Promise<boolean>`**

  Whether the shipping-zones backend is available on this engine build.

- **`create(input: CreateShippingZoneInput): Promise<ShippingZoneOutput>`**

  Types: [`CreateShippingZoneInput`](#createshippingzoneinput), [`ShippingZoneOutput`](#shippingzoneoutput)

- **`get(id: string): Promise<ShippingZoneOutput | null>`**

  Types: [`ShippingZoneOutput`](#shippingzoneoutput)

- **`update(id: string, input: UpdateShippingZoneInput): Promise<ShippingZoneOutput>`**

  Types: [`UpdateShippingZoneInput`](#updateshippingzoneinput), [`ShippingZoneOutput`](#shippingzoneoutput)

- **`list(filter?: ShippingZoneFilterInput | undefined | null): Promise<Array<ShippingZoneOutput>>`**

  Types: [`ShippingZoneFilterInput`](#shippingzonefilterinput), [`ShippingZoneOutput`](#shippingzoneoutput)

- **`delete(id: string): Promise<void>`**

- **`findMatchingZones(country: string, region?: string | undefined | null, postalCode?: string | undefined | null): Promise<Array<ShippingZoneOutput>>`**

  Find zones whose geographic criteria match a destination.

  Types: [`ShippingZoneOutput`](#shippingzoneoutput)

- **`createMethod(input: CreateZoneShippingMethodInput): Promise<ZoneShippingMethodOutput>`**

  Types: [`CreateZoneShippingMethodInput`](#createzoneshippingmethodinput), [`ZoneShippingMethodOutput`](#zoneshippingmethodoutput)

- **`getMethod(id: string): Promise<ZoneShippingMethodOutput | null>`**

  Types: [`ZoneShippingMethodOutput`](#zoneshippingmethodoutput)

- **`listMethods(filter?: ZoneShippingMethodFilterInput | undefined | null): Promise<Array<ZoneShippingMethodOutput>>`**

  Types: [`ZoneShippingMethodFilterInput`](#zoneshippingmethodfilterinput), [`ZoneShippingMethodOutput`](#zoneshippingmethodoutput)

- **`deleteMethod(id: string): Promise<void>`**

- **`calculateRates(request: ZoneShippingRateRequestInput): Promise<Array<ZoneShippingRateOutput>>`**

  Calculate available shipping rates for a destination.

  Types: [`ZoneShippingRateRequestInput`](#zoneshippingraterequestinput), [`ZoneShippingRateOutput`](#zoneshippingrateoutput)

### commerce.stockSnapshots

Class `StockSnapshots`.

- **`isSupported(): Promise<boolean>`**

  Whether the stock-snapshots backend is available on this engine build.

- **`capture(input: CaptureStockSnapshotInput): Promise<StockSnapshotOutput>`**

  Capture a new snapshot; totals are computed from the supplied lines.

  Types: [`CaptureStockSnapshotInput`](#capturestocksnapshotinput), [`StockSnapshotOutput`](#stocksnapshotoutput)

- **`get(id: string): Promise<StockSnapshotOutput | null>`**

  Types: [`StockSnapshotOutput`](#stocksnapshotoutput)

- **`latest(): Promise<StockSnapshotOutput | null>`**

  Most recent snapshot, if any.

  Types: [`StockSnapshotOutput`](#stocksnapshotoutput)

- **`list(filter?: StockSnapshotFilterInput | undefined | null): Promise<Array<StockSnapshotOutput>>`**

  Types: [`StockSnapshotFilterInput`](#stocksnapshotfilterinput), [`StockSnapshotOutput`](#stocksnapshotoutput)

- **`delete(id: string): Promise<void>`**

### commerce.printStations

Class `PrintStations`.

- **`isSupported(): Promise<boolean>`**

  Whether the print-stations backend is available on this engine build.

- **`pair(input: CreatePrintStationInput): Promise<PairStationResultOutput>`**

  Pair a new station, returning the station and its one-time token.

  Types: [`CreatePrintStationInput`](#createprintstationinput), [`PairStationResultOutput`](#pairstationresultoutput)

- **`listStations(filter?: PrintStationFilterInput | undefined | null): Promise<Array<PrintStationOutput>>`**

  List paired stations, optionally filtered by `revoked` and windowed
  by `limit`/`offset`.

  Types: [`PrintStationFilterInput`](#printstationfilterinput), [`PrintStationOutput`](#printstationoutput)

- **`getStation(id: string): Promise<PrintStationOutput | null>`**

  Types: [`PrintStationOutput`](#printstationoutput)

- **`revokeStation(id: string): Promise<PrintStationOutput>`**

  Types: [`PrintStationOutput`](#printstationoutput)

- **`enqueueJob(stationId: string, input: EnqueuePrintJobInput): Promise<PrintJobOutput>`**

  Types: [`EnqueuePrintJobInput`](#enqueueprintjobinput), [`PrintJobOutput`](#printjoboutput)

- **`nextJob(stationId: string): Promise<PrintJobOutput | null>`**

  Pick up the next queued job for a station.

  Types: [`PrintJobOutput`](#printjoboutput)

- **`completeJob(jobId: string, success: boolean): Promise<PrintJobOutput>`**

  Mark a job printed (success) or failed.

  Types: [`PrintJobOutput`](#printjoboutput)

- **`listJobs(stationId: string, filter?: PrintJobFilterInput | undefined | null): Promise<Array<PrintJobOutput>>`**

  Types: [`PrintJobFilterInput`](#printjobfilterinput), [`PrintJobOutput`](#printjoboutput)

### commerce.integrationMappings

Class `IntegrationMappings`.

- **`isSupported(): Promise<boolean>`**

  Whether the integration-mappings backend is available on this engine build.

- **`create(input: CreateIntegrationMappingInput): Promise<IntegrationMappingOutput>`**

  Types: [`CreateIntegrationMappingInput`](#createintegrationmappinginput), [`IntegrationMappingOutput`](#integrationmappingoutput)

- **`get(id: string): Promise<IntegrationMappingOutput | null>`**

  Types: [`IntegrationMappingOutput`](#integrationmappingoutput)

- **`update(id: string, input: UpdateIntegrationMappingInput): Promise<IntegrationMappingOutput>`**

  Types: [`UpdateIntegrationMappingInput`](#updateintegrationmappinginput), [`IntegrationMappingOutput`](#integrationmappingoutput)

- **`list(filter?: IntegrationMappingFilterInput | undefined | null): Promise<Array<IntegrationMappingOutput>>`**

  Types: [`IntegrationMappingFilterInput`](#integrationmappingfilterinput), [`IntegrationMappingOutput`](#integrationmappingoutput)

- **`delete(id: string): Promise<void>`**

- **`bulkUpsert(items: Array<CreateIntegrationMappingInput>): Promise<string>`**

  Bulk upsert mappings; returns the number of rows affected as a string.

  Types: [`CreateIntegrationMappingInput`](#createintegrationmappinginput)

- **`resolve(lookup: MappingLookupInput): Promise<string | null>`**

  Resolve the internal value for an external value.

  Types: [`MappingLookupInput`](#mappinglookupinput)

### commerce.integrationFieldMappings

Class `IntegrationFieldMappings`.

- **`isSupported(): Promise<boolean>`**

  Whether the integration field-mappings backend is available on this engine build.

- **`create(input: CreateIntegrationFieldMappingInput): Promise<IntegrationFieldMappingOutput>`**

  Types: [`CreateIntegrationFieldMappingInput`](#createintegrationfieldmappinginput), [`IntegrationFieldMappingOutput`](#integrationfieldmappingoutput)

- **`get(id: string): Promise<IntegrationFieldMappingOutput | null>`**

  Types: [`IntegrationFieldMappingOutput`](#integrationfieldmappingoutput)

- **`update(id: string, input: UpdateIntegrationFieldMappingInput): Promise<IntegrationFieldMappingOutput>`**

  Types: [`UpdateIntegrationFieldMappingInput`](#updateintegrationfieldmappinginput), [`IntegrationFieldMappingOutput`](#integrationfieldmappingoutput)

- **`list(filter?: IntegrationFieldMappingFilterInput | undefined | null): Promise<Array<IntegrationFieldMappingOutput>>`**

  Types: [`IntegrationFieldMappingFilterInput`](#integrationfieldmappingfilterinput), [`IntegrationFieldMappingOutput`](#integrationfieldmappingoutput)

- **`delete(id: string): Promise<void>`**

- **`bulkCreate(items: Array<CreateIntegrationFieldMappingInput>): Promise<string>`**

  Bulk create field mappings; returns the number of rows affected as a string.

  Types: [`CreateIntegrationFieldMappingInput`](#createintegrationfieldmappinginput)

- **`bulkDelete(ids: Array<string>): Promise<string>`**

  Bulk delete field mappings by ID; returns the number of rows affected as a string.

- **`distinctGroups(integrationAccount: string): Promise<Array<string>>`**

  Distinct mapping groups for an integration account.

### commerce.paymentObligations

Class `PaymentObligations`.

- **`isSupported(): Promise<boolean>`**

  Whether the payment-obligations backend is available on this engine build.

- **`create(input: CreatePaymentObligationInput): Promise<PaymentObligationOutput>`**

  Types: [`CreatePaymentObligationInput`](#createpaymentobligationinput), [`PaymentObligationOutput`](#paymentobligationoutput)

- **`get(id: string): Promise<PaymentObligationOutput | null>`**

  Types: [`PaymentObligationOutput`](#paymentobligationoutput)

- **`list(filter?: PaymentObligationFilterInput | undefined | null): Promise<Array<PaymentObligationOutput>>`**

  Types: [`PaymentObligationFilterInput`](#paymentobligationfilterinput), [`PaymentObligationOutput`](#paymentobligationoutput)

- **`recordPayment(id: string, amount: string): Promise<PaymentObligationOutput>`**

  Record a payment against an obligation.

  Types: [`PaymentObligationOutput`](#paymentobligationoutput)

- **`setStatus(id: string, status: PaymentObligationStatus): Promise<PaymentObligationOutput>`**

  Set the obligation status (e.g. `scheduled`, `cancelled`).

  Types: [`PaymentObligationStatus`](#paymentobligationstatus), [`PaymentObligationOutput`](#paymentobligationoutput)

- **`linkBill(id: string, billId: string): Promise<PaymentObligationOutput>`**

  Link an AP bill to an obligation.

  Types: [`PaymentObligationOutput`](#paymentobligationoutput)

- **`dashboard(today: string): Promise<PaymentObligationDashboardOutput>`**

  Aggregate dashboard summary as of the given date (YYYY-MM-DD).

  Types: [`PaymentObligationDashboardOutput`](#paymentobligationdashboardoutput)

### commerce.maintenance

Class `Maintenance`.

- **`supportsBackup(): Promise<boolean>`**

  Whether file-level backup and restore are available on this instance.

- **`isSupported(): Promise<boolean>`**

  Alias of `supportsBackup`, matching the other accessor modules.

- **`backup(backupPath: string): Promise<BackupReportOutput>`**

  Take a consistent backup to `backupPath`, writing a sidecar manifest.

  Types: [`BackupReportOutput`](#backupreportoutput)

- **`backupTo(backupPath: string): Promise<BackupReportOutput>`**

  Alias of `backup`.

  Types: [`BackupReportOutput`](#backupreportoutput)

- **`restore(backupPath: string, targetPath: string, options?: RestoreOptionsInput | undefined | null): Promise<RestoreReportOutput>`**

  Restore a backup to `targetPath`.

  Types: [`RestoreOptionsInput`](#restoreoptionsinput), [`RestoreReportOutput`](#restorereportoutput)

- **`restoreFrom(backupPath: string, targetPath: string, options?: RestoreOptionsInput | undefined | null): Promise<RestoreReportOutput>`**

  Alias of `restore`.

  Types: [`RestoreOptionsInput`](#restoreoptionsinput), [`RestoreReportOutput`](#restorereportoutput)

- **`export(path: string, options?: ExportOptionsInput | undefined | null): Promise<ExportReportOutput>`**

  Write a structured JSON export to `path`.

  Types: [`ExportOptionsInput`](#exportoptionsinput), [`ExportReportOutput`](#exportreportoutput)

- **`exportToFile(path: string, options?: ExportOptionsInput | undefined | null): Promise<ExportReportOutput>`**

  Alias of `export`.

  Types: [`ExportOptionsInput`](#exportoptionsinput), [`ExportReportOutput`](#exportreportoutput)

- **`import(path: string, options?: ImportOptionsInput | undefined | null): Promise<ImportReportOutput>`**

  Read a structured JSON export from `path` and replay it.

  Types: [`ImportOptionsInput`](#importoptionsinput), [`ImportReportOutput`](#importreportoutput)

- **`importFromFile(path: string, options?: ImportOptionsInput | undefined | null): Promise<ImportReportOutput>`**

  Alias of `import`.

  Types: [`ImportOptionsInput`](#importoptionsinput), [`ImportReportOutput`](#importreportoutput)

- **`exportableDomains(): Promise<Array<string>>`**

  Domains the structured export covers, in export order.

- **`importableDomains(): Promise<Array<string>>`**

  Domains the structured import can write.

- **`listPortableDomains(): Promise<PortableDomainsOutput>`**

  Both portable domain lists in one call.

  Types: [`PortableDomainsOutput`](#portabledomainsoutput)

### commerce.purgatory

Class `Purgatory`.

- **`isSupported(): Promise<boolean>`**

  Whether the purgatory backend is available on this engine build.

- **`ingest(input: IngestOrderInput): Promise<PurgatoryOrderOutput>`**

  Ingest an external order into purgatory.

  Types: [`IngestOrderInput`](#ingestorderinput), [`PurgatoryOrderOutput`](#purgatoryorderoutput)

- **`get(id: string): Promise<PurgatoryOrderOutput | null>`**

  Types: [`PurgatoryOrderOutput`](#purgatoryorderoutput)

- **`list(filter?: PurgatoryFilterInput | undefined | null): Promise<Array<PurgatoryOrderOutput>>`**

  Types: [`PurgatoryFilterInput`](#purgatoryfilterinput), [`PurgatoryOrderOutput`](#purgatoryorderoutput)

- **`mapLine(id: string, lineId: string, input: MapPurgatoryLineInput): Promise<PurgatoryOrderOutput>`**

  Map a staged line to a product and/or toggle its flags.

  Types: [`MapPurgatoryLineInput`](#mappurgatorylineinput), [`PurgatoryOrderOutput`](#purgatoryorderoutput)

- **`post(id: string): Promise<PurgatoryOrderOutput>`**

  Post the order out of purgatory.

  Types: [`PurgatoryOrderOutput`](#purgatoryorderoutput)

- **`delete(id: string): Promise<void>`**

### commerce.topologySnapshots

Class `TopologySnapshots`.

- **`isSupported(): Promise<boolean>`**

  Whether the topology-snapshots backend is available on this engine build.

- **`capture(input: CaptureTopologySnapshotInput): Promise<TopologySnapshotOutput>`**

  Capture a new snapshot; health is derived from the supplied metrics.

  Types: [`CaptureTopologySnapshotInput`](#capturetopologysnapshotinput), [`TopologySnapshotOutput`](#topologysnapshotoutput)

- **`get(id: string): Promise<TopologySnapshotOutput | null>`**

  Types: [`TopologySnapshotOutput`](#topologysnapshotoutput)

- **`latest(): Promise<TopologySnapshotOutput | null>`**

  Most recent snapshot, if any.

  Types: [`TopologySnapshotOutput`](#topologysnapshotoutput)

- **`list(filter?: TopologySnapshotFilterInput | undefined | null): Promise<Array<TopologySnapshotOutput>>`**

  Types: [`TopologySnapshotFilterInput`](#topologysnapshotfilterinput), [`TopologySnapshotOutput`](#topologysnapshotoutput)

- **`delete(id: string): Promise<void>`**

### commerce.fraud

Class `Fraud`.

- **`isSupported(): Promise<boolean>`**

  Whether the fraud backend is available on this engine build.

- **`createAssessment(input: CreateFraudAssessmentInput): Promise<FraudAssessmentOutput>`**

  Create a fraud assessment for an order.

  Types: [`CreateFraudAssessmentInput`](#createfraudassessmentinput), [`FraudAssessmentOutput`](#fraudassessmentoutput)

- **`getAssessment(orderId: string): Promise<FraudAssessmentOutput | null>`**

  Types: [`FraudAssessmentOutput`](#fraudassessmentoutput)

- **`listAssessments(filter?: FraudAssessmentFilterInput | undefined | null): Promise<Array<FraudAssessmentOutput>>`**

  Types: [`FraudAssessmentFilterInput`](#fraudassessmentfilterinput), [`FraudAssessmentOutput`](#fraudassessmentoutput)

- **`reviewAssessment(orderId: string, decision: FraudDecision, reviewer: string, notes?: string | undefined | null): Promise<FraudAssessmentOutput>`**

  Record a manual review decision on an assessment.

  Types: [`FraudDecision`](#frauddecision), [`FraudAssessmentOutput`](#fraudassessmentoutput)

- **`createRule(input: CreateFraudRuleInput): Promise<FraudRuleOutput>`**

  Types: [`CreateFraudRuleInput`](#createfraudruleinput), [`FraudRuleOutput`](#fraudruleoutput)

- **`getRule(id: string): Promise<FraudRuleOutput | null>`**

  Types: [`FraudRuleOutput`](#fraudruleoutput)

- **`updateRule(id: string, input: UpdateFraudRuleInput): Promise<FraudRuleOutput>`**

  Types: [`UpdateFraudRuleInput`](#updatefraudruleinput), [`FraudRuleOutput`](#fraudruleoutput)

- **`listRules(filter?: FraudRuleFilterInput | undefined | null): Promise<Array<FraudRuleOutput>>`**

  Types: [`FraudRuleFilterInput`](#fraudrulefilterinput), [`FraudRuleOutput`](#fraudruleoutput)

- **`deleteRule(id: string): Promise<void>`**

- **`getActiveRules(): Promise<Array<FraudRuleOutput>>`**

  All currently enabled fraud rules.

  Types: [`FraudRuleOutput`](#fraudruleoutput)

### commerce.searchConfig

Class `SearchConfigs`.

- **`isSupported(): Promise<boolean>`**

  Whether the search-configuration backend is available on this engine build.

- **`create(input: CreateSearchConfigInput): Promise<SearchConfigOutput>`**

  Types: [`CreateSearchConfigInput`](#createsearchconfiginput), [`SearchConfigOutput`](#searchconfigoutput)

- **`get(id: string): Promise<SearchConfigOutput | null>`**

  Types: [`SearchConfigOutput`](#searchconfigoutput)

- **`update(id: string, input: UpdateSearchConfigInput): Promise<SearchConfigOutput>`**

  Types: [`UpdateSearchConfigInput`](#updatesearchconfiginput), [`SearchConfigOutput`](#searchconfigoutput)

- **`list(filter?: SearchConfigFilterInput | undefined | null): Promise<Array<SearchConfigOutput>>`**

  Types: [`SearchConfigFilterInput`](#searchconfigfilterinput), [`SearchConfigOutput`](#searchconfigoutput)

- **`delete(id: string): Promise<void>`**

- **`getActive(): Promise<SearchConfigOutput | null>`**

  The currently active search configuration, if any.

  Types: [`SearchConfigOutput`](#searchconfigoutput)

- **`setActive(id: string): Promise<SearchConfigOutput>`**

  Make a configuration active, deactivating the current one.

  Types: [`SearchConfigOutput`](#searchconfigoutput)

### commerce.erc8004

Class `Erc8004`.

- **`registerIdentity(input: CreateAgentIdentityInput): Promise<AgentIdentityOutput>`**

  Register a new agent identity.

  Types: [`CreateAgentIdentityInput`](#createagentidentityinput), [`AgentIdentityOutput`](#agentidentityoutput)

- **`getIdentity(agentRegistry: string, agentId: string): Promise<AgentIdentityOutput | null>`**

  Types: [`AgentIdentityOutput`](#agentidentityoutput)

- **`getIdentityByWallet(agentWallet: string): Promise<AgentIdentityOutput | null>`**

  Types: [`AgentIdentityOutput`](#agentidentityoutput)

- **`updateIdentity(agentRegistry: string, agentId: string, input: UpdateAgentIdentityInput): Promise<AgentIdentityOutput>`**

  Types: [`UpdateAgentIdentityInput`](#updateagentidentityinput), [`AgentIdentityOutput`](#agentidentityoutput)

- **`setAgentWallet(agentRegistry: string, agentId: string, agentWallet: string, proof?: AgentWalletProofInput | undefined | null): Promise<AgentIdentityOutput>`**

  Bind a wallet to an agent identity, with optional on-chain proof data.

  Types: [`AgentWalletProofInput`](#agentwalletproofinput), [`AgentIdentityOutput`](#agentidentityoutput)

- **`clearAgentWallet(agentRegistry: string, agentId: string): Promise<AgentIdentityOutput>`**

  Clear the wallet binding on an agent identity.

  Types: [`AgentIdentityOutput`](#agentidentityoutput)

- **`listIdentities(filter?: AgentIdentityFilterInput | undefined | null): Promise<Array<AgentIdentityOutput>>`**

  Types: [`AgentIdentityFilterInput`](#agentidentityfilterinput), [`AgentIdentityOutput`](#agentidentityoutput)

- **`countIdentities(filter?: AgentIdentityFilterInput | undefined | null): Promise<string>`**

  Count identities matching a filter (returned as a decimal string).

  Types: [`AgentIdentityFilterInput`](#agentidentityfilterinput)

- **`giveFeedback(input: CreateAgentFeedbackInput): Promise<AgentFeedbackOutput>`**

  Give feedback about an agent.

  Types: [`CreateAgentFeedbackInput`](#createagentfeedbackinput), [`AgentFeedbackOutput`](#agentfeedbackoutput)

- **`revokeFeedback(agentRegistry: string, agentId: string, clientAddress: string, feedbackIndex: string): Promise<AgentFeedbackOutput>`**

  Revoke a previously given feedback entry.

  Types: [`AgentFeedbackOutput`](#agentfeedbackoutput)

- **`readFeedback(agentRegistry: string, agentId: string, clientAddress: string, feedbackIndex: string): Promise<AgentFeedbackOutput | null>`**

  Types: [`AgentFeedbackOutput`](#agentfeedbackoutput)

- **`readAllFeedback(filter?: AgentFeedbackFilterInput | undefined | null): Promise<Array<AgentFeedbackOutput>>`**

  Types: [`AgentFeedbackFilterInput`](#agentfeedbackfilterinput), [`AgentFeedbackOutput`](#agentfeedbackoutput)

- **`feedbackSummary(agentRegistry: string, agentId: string, clientAddresses?: Array<string> | undefined | null, tag1?: string | undefined | null, tag2?: string | undefined | null): Promise<FeedbackSummaryOutput>`**

  Aggregate feedback summary for an agent.

  Types: [`FeedbackSummaryOutput`](#feedbacksummaryoutput)

- **`requestValidation(input: CreateAgentValidationRequestInput): Promise<AgentValidationRequestOutput>`**

  Submit a validation request for an agent.

  Types: [`CreateAgentValidationRequestInput`](#createagentvalidationrequestinput), [`AgentValidationRequestOutput`](#agentvalidationrequestoutput)

- **`respondValidation(requestHash: string, input: CreateAgentValidationResponseInput): Promise<AgentValidationResponseOutput>`**

  Record a validator's response to a validation request.

  Types: [`CreateAgentValidationResponseInput`](#createagentvalidationresponseinput), [`AgentValidationResponseOutput`](#agentvalidationresponseoutput)

- **`validationStatus(requestHash: string): Promise<AgentValidationStatusOutput | null>`**

  Types: [`AgentValidationStatusOutput`](#agentvalidationstatusoutput)

- **`validationSummary(agentRegistry: string, agentId: string, validatorAddresses?: Array<string> | undefined | null, tag?: string | undefined | null): Promise<ValidationSummaryOutput>`**

  Aggregate validation summary for an agent.

  Types: [`ValidationSummaryOutput`](#validationsummaryoutput)

### commerce.vendorReturns

Class `VendorReturns`.

- **`isSupported(): Promise<boolean>`**

  Whether the vendor-returns backend is available on this engine build.

- **`create(input: CreateVendorReturnInput): Promise<VendorReturnOutput>`**

  Types: [`CreateVendorReturnInput`](#createvendorreturninput), [`VendorReturnOutput`](#vendorreturnoutput)

- **`get(id: string): Promise<VendorReturnOutput | null>`**

  Types: [`VendorReturnOutput`](#vendorreturnoutput)

- **`list(filter?: VendorReturnFilterInput | undefined | null): Promise<Array<VendorReturnOutput>>`**

  Types: [`VendorReturnFilterInput`](#vendorreturnfilterinput), [`VendorReturnOutput`](#vendorreturnoutput)

- **`submit(id: string): Promise<VendorReturnOutput>`**

  Submit a draft vendor return to the supplier.

  Types: [`VendorReturnOutput`](#vendorreturnoutput)

- **`process(id: string, generateCredit: boolean): Promise<VendorReturnOutput>`**

  Process a vendor return, optionally generating a vendor credit.

  Types: [`VendorReturnOutput`](#vendorreturnoutput)

- **`cancel(id: string): Promise<VendorReturnOutput>`**

  Cancel a vendor return.

  Types: [`VendorReturnOutput`](#vendorreturnoutput)

### commerce.prepayments

Class `Prepayments`.

- **`isSupported(): Promise<boolean>`**

  Whether the prepayments backend is available on this engine build.

- **`create(input: CreatePrepaymentInput): Promise<PrepaymentOutput>`**

  Types: [`CreatePrepaymentInput`](#createprepaymentinput), [`PrepaymentOutput`](#prepaymentoutput)

- **`get(id: string): Promise<PrepaymentOutput | null>`**

  Types: [`PrepaymentOutput`](#prepaymentoutput)

- **`list(filter?: PrepaymentFilterInput | undefined | null): Promise<Array<PrepaymentOutput>>`**

  Types: [`PrepaymentFilterInput`](#prepaymentfilterinput), [`PrepaymentOutput`](#prepaymentoutput)

- **`apply(id: string, input: ApplyPrepaymentInput): Promise<PrepaymentOutput>`**

  Apply a prepayment against a bill or payment obligation.

  Types: [`ApplyPrepaymentInput`](#applyprepaymentinput), [`PrepaymentOutput`](#prepaymentoutput)

- **`listApplications(id: string): Promise<Array<PrepaymentApplicationOutput>>`**

  List applications for a prepayment.

  Types: [`PrepaymentApplicationOutput`](#prepaymentapplicationoutput)

- **`reverseApplication(id: string, applicationId: string): Promise<PrepaymentOutput>`**

  Reverse a previously-recorded application.

  Types: [`PrepaymentOutput`](#prepaymentoutput)

- **`refund(id: string): Promise<PrepaymentOutput>`**

  Refund the remaining balance, closing the prepayment.

  Types: [`PrepaymentOutput`](#prepaymentoutput)

### commerce.vendorCredits

Class `VendorCredits`.

- **`isSupported(): Promise<boolean>`**

  Whether the vendor-credits backend is available on this engine build.

- **`create(input: CreateVendorCreditInput): Promise<VendorCreditOutput>`**

  Types: [`CreateVendorCreditInput`](#createvendorcreditinput), [`VendorCreditOutput`](#vendorcreditoutput)

- **`get(id: string): Promise<VendorCreditOutput | null>`**

  Types: [`VendorCreditOutput`](#vendorcreditoutput)

- **`list(filter?: VendorCreditFilterInput | undefined | null): Promise<Array<VendorCreditOutput>>`**

  Types: [`VendorCreditFilterInput`](#vendorcreditfilterinput), [`VendorCreditOutput`](#vendorcreditoutput)

- **`apply(id: string, input: ApplyVendorCreditInput): Promise<VendorCreditOutput>`**

  Apply a vendor credit against a bill or payment obligation.

  Types: [`ApplyVendorCreditInput`](#applyvendorcreditinput), [`VendorCreditOutput`](#vendorcreditoutput)

- **`listApplications(id: string): Promise<Array<VendorCreditApplicationOutput>>`**

  List applications for a vendor credit.

  Types: [`VendorCreditApplicationOutput`](#vendorcreditapplicationoutput)

- **`reverseApplication(id: string, applicationId: string): Promise<VendorCreditOutput>`**

  Reverse a previously-recorded application.

  Types: [`VendorCreditOutput`](#vendorcreditoutput)

- **`cancel(id: string): Promise<VendorCreditOutput>`**

  Cancel a vendor credit.

  Types: [`VendorCreditOutput`](#vendorcreditoutput)

### commerce.priceSchedules

Class `PriceSchedules`.

- **`isSupported(): Promise<boolean>`**

  Whether the price-schedules backend is available on this engine build.

- **`create(input: CreatePriceScheduleInput): Promise<PriceScheduleOutput>`**

  Types: [`CreatePriceScheduleInput`](#createpricescheduleinput), [`PriceScheduleOutput`](#pricescheduleoutput)

- **`get(id: string): Promise<PriceScheduleOutput | null>`**

  Types: [`PriceScheduleOutput`](#pricescheduleoutput)

- **`update(id: string, input: UpdatePriceScheduleInput): Promise<PriceScheduleOutput>`**

  Types: [`UpdatePriceScheduleInput`](#updatepricescheduleinput), [`PriceScheduleOutput`](#pricescheduleoutput)

- **`list(filter?: PriceScheduleFilterInput | undefined | null): Promise<Array<PriceScheduleOutput>>`**

  Types: [`PriceScheduleFilterInput`](#priceschedulefilterinput), [`PriceScheduleOutput`](#pricescheduleoutput)

- **`delete(id: string): Promise<void>`**

  Delete a price schedule and its entries.

- **`setEntry(id: string, productId: string, price: string): Promise<PriceScheduleEntryOutput>`**

  Upsert a per-product scheduled price (exact decimal string).

  Types: [`PriceScheduleEntryOutput`](#pricescheduleentryoutput)

- **`deleteEntry(id: string, productId: string): Promise<void>`**

  Remove a per-product entry.

- **`listEntries(id: string): Promise<Array<PriceScheduleEntryOutput>>`**

  List per-product entries for a schedule.

  Types: [`PriceScheduleEntryOutput`](#pricescheduleentryoutput)

- **`resolvePrice(productId: string, at?: string | undefined | null): Promise<string | null>`**

  Resolve the effective scheduled price for a product at an instant
  (`at` is an RFC 3339 timestamp; defaults to now). Returns an exact
  decimal string, or null when no schedule applies.

### commerce.priceLevels

Class `PriceLevels`.

- **`isSupported(): Promise<boolean>`**

  Whether the price-levels backend is available on this engine build.

- **`create(input: CreatePriceLevelInput): Promise<PriceLevelOutput>`**

  Types: [`CreatePriceLevelInput`](#createpricelevelinput), [`PriceLevelOutput`](#priceleveloutput)

- **`get(id: string): Promise<PriceLevelOutput | null>`**

  Types: [`PriceLevelOutput`](#priceleveloutput)

- **`update(id: string, input: UpdatePriceLevelInput): Promise<PriceLevelOutput>`**

  Types: [`UpdatePriceLevelInput`](#updatepricelevelinput), [`PriceLevelOutput`](#priceleveloutput)

- **`list(filter?: PriceLevelFilterInput | undefined | null): Promise<Array<PriceLevelOutput>>`**

  Types: [`PriceLevelFilterInput`](#pricelevelfilterinput), [`PriceLevelOutput`](#priceleveloutput)

- **`delete(id: string): Promise<void>`**

  Delete a price level and its entries.

- **`setEntry(id: string, productId: string, price: string): Promise<PriceLevelEntryOutput>`**

  Upsert a per-product fixed price entry (exact decimal string).

  Types: [`PriceLevelEntryOutput`](#pricelevelentryoutput)

- **`deleteEntry(id: string, productId: string): Promise<void>`**

  Remove a per-product entry.

- **`listEntries(id: string): Promise<Array<PriceLevelEntryOutput>>`**

  List per-product entries for a level.

  Types: [`PriceLevelEntryOutput`](#pricelevelentryoutput)

### commerce.transferOrders

Class `TransferOrders`.

- **`isSupported(): Promise<boolean>`**

  Whether the transfer-orders backend is available on this engine build.

- **`create(input: CreateTransferOrderInput): Promise<TransferOrderOutput>`**

  Types: [`CreateTransferOrderInput`](#createtransferorderinput), [`TransferOrderOutput`](#transferorderoutput)

- **`get(id: string): Promise<TransferOrderOutput | null>`**

  Types: [`TransferOrderOutput`](#transferorderoutput)

- **`list(filter?: TransferOrderFilterInput | undefined | null): Promise<Array<TransferOrderOutput>>`**

  Types: [`TransferOrderFilterInput`](#transferorderfilterinput), [`TransferOrderOutput`](#transferorderoutput)

- **`ship(id: string): Promise<TransferOrderOutput>`**

  Mark a transfer order as shipped from the source.

  Types: [`TransferOrderOutput`](#transferorderoutput)

- **`receiveLine(id: string, itemId: string, quantity: string): Promise<TransferOrderOutput>`**

  Receive a quantity (exact decimal string) against a single line at the
  destination.

  Types: [`TransferOrderOutput`](#transferorderoutput)

- **`cancel(id: string): Promise<TransferOrderOutput>`**

  Cancel a transfer order.

  Types: [`TransferOrderOutput`](#transferorderoutput)

### commerce.productionBatches

Class `ProductionBatches`.

- **`isSupported(): Promise<boolean>`**

  Whether the production-batches backend is available on this engine build.

- **`create(input: CreateProductionBatchInput): Promise<ProductionBatchOutput>`**

  Types: [`CreateProductionBatchInput`](#createproductionbatchinput), [`ProductionBatchOutput`](#productionbatchoutput)

- **`get(id: string): Promise<ProductionBatchOutput | null>`**

  Types: [`ProductionBatchOutput`](#productionbatchoutput)

- **`update(id: string, input: UpdateProductionBatchInput): Promise<ProductionBatchOutput>`**

  Types: [`UpdateProductionBatchInput`](#updateproductionbatchinput), [`ProductionBatchOutput`](#productionbatchoutput)

- **`list(filter?: ProductionBatchFilterInput | undefined | null): Promise<Array<ProductionBatchOutput>>`**

  Types: [`ProductionBatchFilterInput`](#productionbatchfilterinput), [`ProductionBatchOutput`](#productionbatchoutput)

- **`delete(id: string): Promise<void>`**

  Delete a production batch.

- **`addWorkOrders(id: string, workOrderIds: Array<string>): Promise<ProductionBatchOutput>`**

  Link work orders to a batch.

  Types: [`ProductionBatchOutput`](#productionbatchoutput)

- **`removeWorkOrder(id: string, workOrderId: string): Promise<ProductionBatchOutput>`**

  Remove a work order from a batch.

  Types: [`ProductionBatchOutput`](#productionbatchoutput)

### commerce.supplierSkus

Class `SupplierSkus`.

- **`isSupported(): Promise<boolean>`**

  Whether the supplier-SKUs backend is available on this engine build.

- **`create(input: CreateSupplierSkuInput): Promise<SupplierSkuOutput>`**

  Types: [`CreateSupplierSkuInput`](#createsupplierskuinput), [`SupplierSkuOutput`](#supplierskuoutput)

- **`get(id: string): Promise<SupplierSkuOutput | null>`**

  Types: [`SupplierSkuOutput`](#supplierskuoutput)

- **`update(id: string, input: UpdateSupplierSkuInput): Promise<SupplierSkuOutput>`**

  Types: [`UpdateSupplierSkuInput`](#updatesupplierskuinput), [`SupplierSkuOutput`](#supplierskuoutput)

- **`list(filter?: SupplierSkuFilterInput | undefined | null): Promise<Array<SupplierSkuOutput>>`**

  Types: [`SupplierSkuFilterInput`](#supplierskufilterinput), [`SupplierSkuOutput`](#supplierskuoutput)

- **`delete(id: string): Promise<void>`**

  Delete a supplier SKU.

- **`bulkUpsert(supplierId: string, items: Array<BulkSupplierSkuItemInput>): Promise<number>`**

  Bulk upsert supplier SKUs for a supplier, keyed by internal product.
  Returns the number of records upserted.

  Types: [`BulkSupplierSkuItemInput`](#bulksupplierskuiteminput)

### commerce.inboundShipments

Class `InboundShipments`.

- **`isSupported(): Promise<boolean>`**

  Whether the inbound-shipments backend is available on this engine build.

- **`create(input: CreateInboundShipmentInput): Promise<InboundShipmentOutput>`**

  Types: [`CreateInboundShipmentInput`](#createinboundshipmentinput), [`InboundShipmentOutput`](#inboundshipmentoutput)

- **`get(id: string): Promise<InboundShipmentOutput | null>`**

  Types: [`InboundShipmentOutput`](#inboundshipmentoutput)

- **`list(filter?: InboundShipmentFilterInput | undefined | null): Promise<Array<InboundShipmentOutput>>`**

  Types: [`InboundShipmentFilterInput`](#inboundshipmentfilterinput), [`InboundShipmentOutput`](#inboundshipmentoutput)

- **`markInTransit(id: string): Promise<InboundShipmentOutput>`**

  Mark a shipment as in transit.

  Types: [`InboundShipmentOutput`](#inboundshipmentoutput)

- **`markArrived(id: string): Promise<InboundShipmentOutput>`**

  Mark a shipment as arrived.

  Types: [`InboundShipmentOutput`](#inboundshipmentoutput)

- **`receiveLine(id: string, itemId: string, quantity: string): Promise<InboundShipmentOutput>`**

  Receive a quantity (exact decimal string) against a single line.

  Types: [`InboundShipmentOutput`](#inboundshipmentoutput)

- **`cancel(id: string): Promise<InboundShipmentOutput>`**

  Cancel an inbound shipment.

  Types: [`InboundShipmentOutput`](#inboundshipmentoutput)

### commerce.events

Class `Events`.

- **`subscribe(): Promise<CommerceEventSubscription>`**

  Subscribe to all commerce events.

  Types: [`CommerceEventSubscription`](#commerceeventssubscribe)

- **`subscribeFiltered(eventTypes: Array<string>): Promise<CommerceEventSubscription>`**

  Subscribe to a subset of commerce events by event type.

  Event types must match `CommerceEvent::event_type()` values (snake_case),
  e.g. "order_created", "inventory_adjusted".

  Types: [`CommerceEventSubscription`](#commerceeventssubscribe)

- **`listWebhooks(): Promise<Array<WebhookOutput>>`**

  List registered webhooks.

  Types: [`WebhookOutput`](#webhookoutput)

- **`registerWebhook(input: CreateWebhookInput): Promise<string | null>`**

  Register a webhook endpoint for event delivery.

  Types: [`CreateWebhookInput`](#createwebhookinput)

- **`unregisterWebhook(id: string): Promise<boolean>`**

  Unregister a webhook endpoint.

### commerce.vector(apiKey)

Class `VectorSearch`.

Vector search operations for semantic similarity search

- **`searchProducts(query: string, limit?: number | undefined | null): Promise<Array<ProductSearchResultOutput>>`**

  Search products using natural language query

  Types: [`ProductSearchResultOutput`](#productsearchresultoutput)

- **`searchCustomers(query: string, limit?: number | undefined | null): Promise<Array<CustomerSearchResultOutput>>`**

  Search customers using natural language query

  Types: [`CustomerSearchResultOutput`](#customersearchresultoutput)

- **`searchOrders(query: string, limit?: number | undefined | null): Promise<Array<OrderSearchResultOutput>>`**

  Search orders using natural language query

  Types: [`OrderSearchResultOutput`](#ordersearchresultoutput)

- **`searchInventory(query: string, limit?: number | undefined | null): Promise<Array<InventorySearchResultOutput>>`**

  Search inventory items using natural language query

  Types: [`InventorySearchResultOutput`](#inventorysearchresultoutput)

- **`indexProduct(productId: string): Promise<void>`**

  Index a product for vector search

- **`indexCustomer(customerId: string): Promise<void>`**

  Index a customer for vector search

- **`indexOrder(orderId: string): Promise<void>`**

  Index an order for vector search

- **`indexInventoryItem(itemId: string): Promise<void>`**

  Index an inventory item for vector search

- **`indexAllProducts(): Promise<number>`**

  Index all products for vector search

- **`indexAllCustomers(): Promise<number>`**

  Index all customers for vector search

- **`indexAllOrders(): Promise<number>`**

  Index all orders for vector search

- **`indexAllInventory(): Promise<number>`**

  Index all inventory items for vector search

- **`stats(): Promise<EmbeddingStatsOutput>`**

  Get embedding statistics

  Types: [`EmbeddingStatsOutput`](#embeddingstatsoutput)

- **`clear(entityType: VectorEntityType): Promise<number>`**

  Clear all embeddings for a specific entity type

  Types: [`VectorEntityType`](#vectorentitytype)

- **`clearAll(): Promise<number>`**

  Clear all embeddings

### commerce.events.subscribe()

Class `CommerceEventSubscription`. Extends `AsyncIterable<CommerceEvent>`. Also available as `commerce.events.subscribeFiltered(eventTypes)`.

A stream of commerce events.

Delivery runs on the binding's runtime and hands each event to a JavaScript
callback through a threadsafe function that starts *unreferenced*, so a
subscription never keeps the process alive by itself. (Every napi async
method resolves its promise through a *referenced* threadsafe function,
which is why the previous `recv()` — a native promise that could stay
pending forever — pinned the event loop at exit.) The `recv()` and
async-iterator surface lives in `index.js`, on top of `__start`.

- **`get isClosed: boolean`**

  Whether the stream has ended, by `close()` or because the engine closed.

- **`__start(callback: (event: any | null) => void): void`**

  Start delivery to `callback`. Each event arrives as a JSON object with
  an `event_type` field; `null` means the stream has ended. Internal —
  `index.js` calls this once, lazily, and exposes `recv()` and async
  iteration on top.

- **`close(): void`**

  End the stream. A pending `recv()` resolves `null`, and so does every
  later one. Idempotent.

- **`ref(): void`**

  Keep the process alive while this subscription is open. Off by
  default: an open subscription alone never prevents exit.

- **`unref(): void`**

  Undo `ref()`.

- **`recv(): Promise<CommerceEvent | null>`**

  The next event, or `null` once the stream has ended (after `close()`, or
  when the owning `Commerce` closes).

  An open subscription never keeps the process alive by itself; call
  `ref()` if it should.

  Types: [`CommerceEvent`](#commerceevent)

- **`[Symbol.asyncIterator](): AsyncIterator<CommerceEvent>`**

  Iterate events until the stream ends. Leaving the loop calls `close()`.

  Types: [`CommerceEvent`](#commerceevent)

## Functions

Free functions exported by the package (cryptography, VES envelopes, x402 signing).

### vesStrictGenerateSigningKeypair

```ts
function vesStrictGenerateSigningKeypair(): StrictSigningKeypairOutput
```

Generate an ML-DSA-65-only signing keypair for PQC-strict mode.

Types: [`StrictSigningKeypairOutput`](#strictsigningkeypairoutput)

### vesStrictSignEventHash

```ts
function vesStrictSignEventHash(hash: Buffer, mlDsa65Seed: Buffer): Buffer
```

Sign a 32-byte hash with ML-DSA-65 only (PQC-strict mode).

### vesStrictVerifyEventSignature

```ts
function vesStrictVerifyEventSignature(hash: Buffer, mlDsa65Signature: Buffer, mlDsa65PublicKey: Buffer): boolean
```

Verify a 32-byte hash with ML-DSA-65 only (PQC-strict mode).

### vesStrictGenerateRecipientKeypair

```ts
function vesStrictGenerateRecipientKeypair(kid: number): StrictRecipientKeypairOutput
```

Generate an ML-KEM-768-only recipient keypair for PQC-strict mode.

Types: [`StrictRecipientKeypairOutput`](#strictrecipientkeypairoutput)

### vesStrictEncryptPayload

```ts
function vesStrictEncryptPayload(payloadJson: string, aadParams: HybridPayloadAadParamsInput, recipients: Array<StrictRecipientPublicKeyInput>): StrictEncryptionResultOutput
```

Encrypt a JSON payload using ML-KEM-768-only recipient wrapping (PQC-strict).

Types: [`HybridPayloadAadParamsInput`](#hybridpayloadaadparamsinput), [`StrictRecipientPublicKeyInput`](#strictrecipientpublickeyinput), [`StrictEncryptionResultOutput`](#strictencryptionresultoutput)

### vesStrictDecryptPayload

```ts
function vesStrictDecryptPayload(payloadEncryptedJson: string, payloadAad: Buffer, recipientKid: number, recipientPrivateKey: StrictRecipientPrivateKeyInput, expectedPlainHash: Buffer): string
```

Decrypt a JSON payload using ML-KEM-768-only recipient wrapping (PQC-strict).

Types: [`StrictRecipientPrivateKeyInput`](#strictrecipientprivatekeyinput)

### vesHybridGenerateSigningPop

```ts
function vesHybridGenerateSigningPop(ed25519PrivateKey: Buffer, mlDsa65Seed: Buffer, ed25519PublicKey: Buffer, mlDsa65PublicKey: Buffer): HybridSignatureBundleOutput
```

Generate a hybrid signing proof-of-possession bundle.

Types: [`HybridSignatureBundleOutput`](#hybridsignaturebundleoutput)

### vesHybridVerifySigningPop

```ts
function vesHybridVerifySigningPop(ed25519Signature: Buffer, mlDsa65Signature: Buffer, ed25519PublicKey: Buffer, mlDsa65PublicKey: Buffer): boolean
```

Verify a hybrid signing proof-of-possession bundle.

### vesStrictGenerateSigningPop

```ts
function vesStrictGenerateSigningPop(mlDsa65Seed: Buffer, mlDsa65PublicKey: Buffer): Buffer
```

Generate a PQC-strict signing proof-of-possession.

### vesStrictVerifySigningPop

```ts
function vesStrictVerifySigningPop(mlDsa65Signature: Buffer, mlDsa65PublicKey: Buffer): boolean
```

Verify a PQC-strict signing proof-of-possession.

### aesGcmEncrypt

```ts
function aesGcmEncrypt(plaintext: Buffer, key: Buffer, aad: Buffer): Buffer
```

Encrypt a buffer with AES-256-GCM

Returns nonce (12 bytes) || ciphertext || tag (16 bytes)

### aesGcmDecrypt

```ts
function aesGcmDecrypt(encrypted: Buffer, key: Buffer, aad: Buffer): Buffer
```

Decrypt a buffer with AES-256-GCM

Input: nonce (12 bytes) || ciphertext || tag (16 bytes)

### merkleRoot

```ts
function merkleRoot(leaves: Array<Buffer>): Buffer
```

Compute Merkle root from an array of 32-byte leaf hashes

### jcsCanonicalize

```ts
function jcsCanonicalize(jsonStr: string): string
```

Canonicalize a JSON string per RFC 8785 JCS

### domainHash

```ts
function domainHash(domain: VesHashDomain, data: Buffer): Buffer
```

Compute domain-separated SHA-256 hash

domain: one of "PAYLOAD_PLAIN", "PAYLOAD_AAD", "PAYLOAD_CIPHER", "RECIPIENTS",
        "EVENTSIG", "LEAF", "NODE", "PAD_LEAF", "STREAM", "RECEIPT"
data: hex-encoded data to hash (after the domain prefix)

Types: [`VesHashDomain`](#veshashdomain)

### ed25519Sign

```ts
function ed25519Sign(hash: Buffer, privateKey: Buffer): Buffer
```

Sign a 32-byte hash with Ed25519

Returns 64-byte signature

### ed25519Verify

```ts
function ed25519Verify(hash: Buffer, signature: Buffer, publicKey: Buffer): boolean
```

Verify an Ed25519 signature

Returns true if signature is valid

### vesHybridGenerateSigningKeypair

```ts
function vesHybridGenerateSigningKeypair(): HybridSigningKeypairOutput
```

Generate a hybrid `Ed25519 + ML-DSA-65` signing keypair.

Types: [`HybridSigningKeypairOutput`](#hybridsigningkeypairoutput)

### vesHybridSignEventHash

```ts
function vesHybridSignEventHash(hash: Buffer, ed25519PrivateKey: Buffer, mlDsa65Seed: Buffer): HybridSignatureBundleOutput
```

Sign a 32-byte hash with the hybrid `Ed25519 + ML-DSA-65` profile.

Types: [`HybridSignatureBundleOutput`](#hybridsignaturebundleoutput)

### vesHybridVerifyEventSignature

```ts
function vesHybridVerifyEventSignature(hash: Buffer, ed25519Signature: Buffer, mlDsa65Signature: Buffer, ed25519PublicKey: Buffer, mlDsa65PublicKey: Buffer): boolean
```

Verify a 32-byte hash with the hybrid `Ed25519 + ML-DSA-65` profile.

### vesTestVectorMlDsaPublicKey

```ts
function vesTestVectorMlDsaPublicKey(): Buffer
```

Return the fixed-seed ML-DSA-65 public key used by cross-language test vectors.

### vesHybridGenerateRecipientKeypair

```ts
function vesHybridGenerateRecipientKeypair(kid: number): HybridRecipientKeypairOutput
```

Generate a hybrid `X25519 + ML-KEM-768` recipient keypair.

Types: [`HybridRecipientKeypairOutput`](#hybridrecipientkeypairoutput)

### vesTestVectorMlKemPublicKey

```ts
function vesTestVectorMlKemPublicKey(): Buffer
```

Return the fixed-seed ML-KEM-768 public key used by cross-language test vectors.

### vesHybridEncryptPayload

```ts
function vesHybridEncryptPayload(payloadJson: string, aadParams: HybridPayloadAadParamsInput, recipients: Array<HybridRecipientPublicKeyInput>): HybridEncryptionResultOutput
```

Encrypt a JSON payload using hybrid `X25519 + ML-KEM-768` recipient wrapping.

Types: [`HybridPayloadAadParamsInput`](#hybridpayloadaadparamsinput), [`HybridRecipientPublicKeyInput`](#hybridrecipientpublickeyinput), [`HybridEncryptionResultOutput`](#hybridencryptionresultoutput)

### vesHybridDecryptPayload

```ts
function vesHybridDecryptPayload(payloadEncryptedJson: string, payloadAad: Buffer, recipientKid: number, recipientPrivateKey: HybridRecipientPrivateKeyInput, expectedPlainHash: Buffer): string
```

Decrypt a JSON payload using hybrid `X25519 + ML-KEM-768` recipient wrapping.

Types: [`HybridRecipientPrivateKeyInput`](#hybridrecipientprivatekeyinput)

### vesX402ComputeSigningHash

```ts
function vesX402ComputeSigningHash(input: X402SigningHashInput): Buffer
```

Compute the sequencer-compatible x402 signing hash for a payment intent shape.

Types: [`X402SigningHashInput`](#x402signinghashinput)

## Types

Interfaces, type aliases and enums, alphabetically. Optional fields are marked `?`.

### ActivityLogEntryOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `subjectType` | `string` |  |
| `subjectId` | `string` |  |
| `action` | `string` |  |
| `summary` | `string` |  |
| `actorKind` | `ActorKind` | user, system, integration, agent Types: [`ActorKind`](#actorkind) |
| `actor?` | `string` |  |
| `metadata` | `string` | Metadata as JSON |
| `createdAt` | `string` |  |

### ActivityLogFilterInput

| Field | Type | Description |
|---|---|---|
| `subjectType?` | `string` |  |
| `subjectId?` | `string` |  |
| `action?` | `string` |  |
| `actorKind?` | `ActorKind` | user, system, integration, agent Types: [`ActorKind`](#actorkind) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### ActorKind

Kind of actor that produced an activity log entry (`ActorKind`).

```ts
type ActorKind = 'user' | 'system' | 'integration' | 'agent'
```

One of: `'user'`, `'system'`, `'integration'`, `'agent'`.

### AddCartItemExactInput

Exact-money cart item input. Monetary values are base-10 strings.

| Field | Type | Description |
|---|---|---|
| `productId?` | `string` |  |
| `variantId?` | `string` |  |
| `sku` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `imageUrl?` | `string` |  |
| `quantity` | `number` |  |
| `unitPrice` | `string` |  |
| `originalPrice?` | `string` |  |
| `weight?` | `string` |  |
| `requiresShipping?` | `boolean` |  |

### AddCartItemInput

| Field | Type | Description |
|---|---|---|
| `productId?` | `string` |  |
| `variantId?` | `string` |  |
| `sku` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `imageUrl?` | `string` |  |
| `quantity` | `number` |  |
| `unitPrice?` | `number` | Float unit price. Optional: send `unit_price_exact` instead for exact money. |
| `unitPriceExact?` | `string` | Exact base-10 unit price. Takes precedence over `unit_price` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `originalPrice?` | `number` |  |
| `originalPriceExact?` | `string` | Exact base-10 original price. Takes precedence over `original_price` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `weight?` | `number` |  |
| `requiresShipping?` | `boolean` |  |

### AddWishlistItemInput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `variantId?` | `string` |  |
| `note?` | `string` |  |
| `quantity?` | `number` |  |
| `priority?` | `number` |  |

### AddressType

Which role(s) an address serves (`CustomerAddressOutput.addressType`, `CreateCustomerAddressInput.addressType`, `setDefaultAddress`).

```ts
type AddressType = 'shipping' | 'billing' | 'both'
```

One of: `'shipping'`, `'billing'`, `'both'`.

### AdjustPointsInput

| Field | Type | Description |
|---|---|---|
| `accountId` | `string` |  |
| `points` | `number` |  |
| `transactionType` | `LoyaltyTransactionType` | Transaction type, e.g. "earn", "redeem", "adjust", "expire" Types: [`LoyaltyTransactionType`](#loyaltytransactiontype) |
| `referenceId?` | `string` |  |
| `description?` | `string` |  |

### AdjustStoreCreditInput

| Field | Type | Description |
|---|---|---|
| `amount` | `string` | Signed adjustment as an exact decimal string ("10.00" adds, "-10.00" subtracts). The balance may not be driven below zero. |
| `note?` | `string` |  |
| `referenceId?` | `string` |  |

### AgentFeedbackFilterInput

| Field | Type | Description |
|---|---|---|
| `agentRegistry?` | `string` |  |
| `agentId?` | `string` |  |
| `clientAddresses?` | `Array<string>` |  |
| `tag1?` | `string` |  |
| `tag2?` | `string` |  |
| `includeRevoked?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### AgentFeedbackOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `agentRegistry` | `string` |  |
| `agentId` | `string` |  |
| `clientAddress` | `string` |  |
| `feedbackIndex` | `string` | Feedback index as a decimal string |
| `value` | `string` | Signed integer value as a decimal string |
| `valueDecimals` | `number` |  |
| `tag1?` | `string` |  |
| `tag2?` | `string` |  |
| `endpoint?` | `string` |  |
| `feedbackUri?` | `string` |  |
| `feedbackHash?` | `string` |  |
| `isRevoked` | `boolean` |  |
| `createdAt` | `string` |  |
| `revokedAt?` | `string` |  |

### AgentIdentityFilterInput

| Field | Type | Description |
|---|---|---|
| `agentRegistry?` | `string` |  |
| `agentId?` | `string` |  |
| `agentWallet?` | `string` |  |
| `ownerAddress?` | `string` |  |
| `agentCardId?` | `string` |  |
| `active?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### AgentIdentityOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `agentRegistry` | `string` |  |
| `agentId` | `string` |  |
| `agentUri` | `string` |  |
| `agentWallet?` | `string` |  |
| `ownerAddress?` | `string` |  |
| `agentCardId?` | `string` |  |
| `registration?` | `string` | JSON-encoded registration document |
| `registrationHash?` | `string` |  |
| `walletProofType?` | `AgentWalletProofType` | Snake-case proof type Types: [`AgentWalletProofType`](#agentwalletprooftype) |
| `walletProof?` | `string` |  |
| `walletProofChainId?` | `string` | Chain id as a decimal string |
| `walletProofDeadline?` | `string` |  |
| `active` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### AgentValidationRequestOutput

| Field | Type | Description |
|---|---|---|
| `requestHash` | `string` |  |
| `agentRegistry` | `string` |  |
| `agentId` | `string` |  |
| `validatorAddress` | `string` |  |
| `requestUri` | `string` |  |
| `createdAt` | `string` |  |

### AgentValidationResponseOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `requestHash` | `string` |  |
| `agentRegistry` | `string` |  |
| `agentId` | `string` |  |
| `validatorAddress` | `string` |  |
| `response` | `number` |  |
| `responseUri?` | `string` |  |
| `responseHash?` | `string` |  |
| `tag?` | `string` |  |
| `createdAt` | `string` |  |

### AgentValidationStatusOutput

| Field | Type | Description |
|---|---|---|
| `validatorAddress` | `string` |  |
| `agentRegistry` | `string` |  |
| `agentId` | `string` |  |
| `response` | `number` |  |
| `responseHash?` | `string` |  |
| `tag?` | `string` |  |
| `lastUpdate` | `string` |  |

### AgentWalletProofInput

Optional on-chain proof data accompanying a wallet binding.

| Field | Type | Description |
|---|---|---|
| `proofType?` | `AgentWalletProofTypeInput` | Snake-case proof type Types: [`AgentWalletProofTypeInput`](#agentwalletprooftypeinput) |
| `proof?` | `string` |  |
| `proofChainId?` | `string` | Chain id as a decimal string |
| `proofDeadline?` | `string` | RFC3339 timestamp |

### AgentWalletProofType

ERC-8004 wallet proof type as rendered on an identity (`AgentWalletProofType`).

```ts
type AgentWalletProofType = 'eip_712' | 'erc_1271'
```

One of: `'eip_712'`, `'erc_1271'`.

### AgentWalletProofTypeInput

ERC-8004 wallet proof type spellings the parser accepts (exact case); records read back as `AgentWalletProofType`.

```ts
type AgentWalletProofTypeInput = 'eip712' | 'eip_712' | 'erc1271' | 'erc_1271'
```

One of: `'eip712'`, `'eip_712'`, `'erc1271'`, `'erc_1271'`.

### AnalyticsGranularity

Bucket size accepted by `AnalyticsQueryInput.granularity` and `revenueForecast`; an unrecognised value is refused with `VALIDATION`.

```ts
type AnalyticsGranularity = 'hour' | 'hourly' | 'day' | 'daily' | 'week' | 'weekly' | 'month' | 'monthly' | 'quarter' | 'quarterly' | 'year' | 'yearly'
```

One of: `'hour'`, `'hourly'`, `'day'`, `'daily'`, `'week'`, `'weekly'`, `'month'`, `'monthly'`, `'quarter'`, `'quarterly'`, `'year'`, `'yearly'`.

### AnalyticsPeriod

Reporting window accepted by `AnalyticsQueryInput.period`; an unrecognised value is refused with `VALIDATION`.

```ts
type AnalyticsPeriod = 'today' | 'yesterday' | 'last7days' | 'last_7_days' | 'last30days' | 'last_30_days' | 'this_month' | 'thismonth' | 'last_month' | 'lastmonth' | 'this_quarter' | 'thisquarter' | 'last_quarter' | 'lastquarter' | 'this_year' | 'thisyear' | 'last_year' | 'lastyear' | 'all_time' | 'alltime' | 'all'
```

One of: `'today'`, `'yesterday'`, `'last7days'`, `'last_7_days'`, `'last30days'`, `'last_30_days'`, `'this_month'`, `'thismonth'`, `'last_month'`, `'lastmonth'`, `'this_quarter'`, `'thisquarter'`, `'last_quarter'`, `'lastquarter'`, `'this_year'`, `'thisyear'`, `'last_year'`, `'lastyear'`, `'all_time'`, `'alltime'`, `'all'`.

### AnalyticsQueryInput

| Field | Type | Description |
|---|---|---|
| `period?` | `AnalyticsPeriod` | Reporting window. Defaults to the last 30 days. Types: [`AnalyticsPeriod`](#analyticsperiod) |
| `granularity?` | `AnalyticsGranularity` | Bucket size for period breakdowns. Defaults to `day`. Types: [`AnalyticsGranularity`](#analyticsgranularity) |
| `limit?` | `number` | Maximum results |

### ApAgingSummaryOutput

| Field | Type | Description |
|---|---|---|
| `current` | `number` | **Deprecated.** Use the `currentExact` twin; float money will be removed in 2.0. |
| `currentExact` | `string` | Exact base-10 current, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `days130` | `number` | **Deprecated.** Use the `days130Exact` twin; float money will be removed in 2.0. |
| `days130Exact` | `string` | Exact base-10 days 1 30, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `days3160` | `number` | **Deprecated.** Use the `days3160Exact` twin; float money will be removed in 2.0. |
| `days3160Exact` | `string` | Exact base-10 days 31 60, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `days6190` | `number` | **Deprecated.** Use the `days6190Exact` twin; float money will be removed in 2.0. |
| `days6190Exact` | `string` | Exact base-10 days 61 90, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `daysOver90` | `number` | **Deprecated.** Use the `daysOver90Exact` twin; float money will be removed in 2.0. |
| `daysOver90Exact` | `string` | Exact base-10 days over 90, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `total` | `number` | **Deprecated.** Use the `totalExact` twin; float money will be removed in 2.0. |
| `totalExact` | `string` | Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### AppliedPromotionOutput

An applied promotion

| Field | Type | Description |
|---|---|---|
| `promotionId` | `string` |  |
| `promotionName` | `string` |  |
| `couponCode?` | `string` |  |
| `discountAmount` | `number` | **Deprecated.** Use the `discountAmountExact` twin; float money will be removed in 2.0. |
| `discountAmountExact` | `string` | Exact base-10 discount amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `discountType` | `PromotionType` | Types: [`PromotionType`](#promotiontype) |

### ApplyPrepaymentInput

| Field | Type | Description |
|---|---|---|
| `targetType` | `PrepaymentTargetType` | bill or payment_obligation Types: [`PrepaymentTargetType`](#prepaymenttargettype) |
| `targetId` | `string` |  |
| `amount` | `string` | Exact decimal string |

### ApplyPromotionsInput

Input for applying promotions

| Field | Type | Description |
|---|---|---|
| `cartId?` | `string` |  |
| `customerId?` | `string` |  |
| `couponCodes?` | `Array<string>` |  |
| `lineItems` | `Array<PromotionLineItemInput>` | Types: [`PromotionLineItemInput`](#promotionlineiteminput) |
| `subtotal` | `number` |  |
| `shippingAmount?` | `number` |  |
| `shippingCountry?` | `string` |  |
| `shippingState?` | `string` |  |
| `currency?` | `string` |  |

### ApplyPromotionsOutput

Result of applying promotions

| Field | Type | Description |
|---|---|---|
| `originalSubtotal` | `number` | **Deprecated.** Use the `originalSubtotalExact` twin; float money will be removed in 2.0. |
| `originalSubtotalExact` | `string` | Exact base-10 original subtotal, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `totalDiscount` | `number` | **Deprecated.** Use the `totalDiscountExact` twin; float money will be removed in 2.0. |
| `totalDiscountExact` | `string` | Exact base-10 total discount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `discountedSubtotal` | `number` | **Deprecated.** Use the `discountedSubtotalExact` twin; float money will be removed in 2.0. |
| `discountedSubtotalExact` | `string` | Exact base-10 discounted subtotal, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `originalShipping` | `number` | **Deprecated.** Use the `originalShippingExact` twin; float money will be removed in 2.0. |
| `originalShippingExact` | `string` | Exact base-10 original shipping, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `shippingDiscount` | `number` | **Deprecated.** Use the `shippingDiscountExact` twin; float money will be removed in 2.0. |
| `shippingDiscountExact` | `string` | Exact base-10 shipping discount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `finalShipping` | `number` | **Deprecated.** Use the `finalShippingExact` twin; float money will be removed in 2.0. |
| `finalShippingExact` | `string` | Exact base-10 final shipping, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `grandTotal` | `number` | **Deprecated.** Use the `grandTotalExact` twin; float money will be removed in 2.0. |
| `grandTotalExact` | `string` | Exact base-10 grand total, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `appliedPromotions` | `Array<AppliedPromotionOutput>` | Types: [`AppliedPromotionOutput`](#appliedpromotionoutput) |

### ApplyVendorCreditInput

| Field | Type | Description |
|---|---|---|
| `targetType` | `VendorCreditTargetType` | bill or payment_obligation Types: [`VendorCreditTargetType`](#vendorcredittargettype) |
| `targetId` | `string` |  |
| `amount` | `string` | Exact decimal string |

### ArAgingSummaryOutput

| Field | Type | Description |
|---|---|---|
| `current` | `number` | **Deprecated.** Use the `currentExact` twin; float money will be removed in 2.0. |
| `currentExact` | `string` | Exact base-10 current, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `days130` | `number` | **Deprecated.** Use the `days130Exact` twin; float money will be removed in 2.0. |
| `days130Exact` | `string` | Exact base-10 days 1 30, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `days3160` | `number` | **Deprecated.** Use the `days3160Exact` twin; float money will be removed in 2.0. |
| `days3160Exact` | `string` | Exact base-10 days 31 60, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `days6190` | `number` | **Deprecated.** Use the `days6190Exact` twin; float money will be removed in 2.0. |
| `days6190Exact` | `string` | Exact base-10 days 61 90, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `daysOver90` | `number` | **Deprecated.** Use the `daysOver90Exact` twin; float money will be removed in 2.0. |
| `daysOver90Exact` | `string` | Exact base-10 days over 90, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `total` | `number` | **Deprecated.** Use the `totalExact` twin; float money will be removed in 2.0. |
| `totalExact` | `string` | Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### AssetAmountWire

Exact non-fiat amount identified by an asset symbol or chain-qualified
token id (`AssetAmountWire`). The asset id is case-sensitive.

| Field | Type | Description |
|---|---|---|
| `amount` | `string` |  |
| `asset` | `string` |  |

### AssetDisposalOutput

| Field | Type | Description |
|---|---|---|
| `disposalDate` | `string` | ISO date (YYYY-MM-DD) |
| `proceeds` | `string` | Exact decimal string |
| `bookValueAtDisposal` | `string` | Exact decimal string |
| `gainLoss` | `string` | Exact decimal string: proceeds - book value |
| `notes?` | `string` |  |

### BackorderFilterInput

Optional filters for `Backorders.listBackorders`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `orderId?` | `string` |  |
| `customerId?` | `string` |  |
| `sku?` | `string` |  |
| `status?` | `BackorderStatusInput` | The rendered form (`ReadyToShip`) or the engine's snake_case (`ready_to_ship`). Types: [`BackorderStatusInput`](#backorderstatusinput) |
| `priority?` | `BackorderPriorityFilter` | The rendered form (`High`) or lowercase (`high`). Types: [`BackorderPriorityFilter`](#backorderpriorityfilter) |
| `expectedBefore?` | `string` | RFC 3339 timestamp. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### BackorderOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `backorderNumber` | `string` |  |
| `orderId` | `string` |  |
| `customerId` | `string` |  |
| `sku` | `string` |  |
| `quantityOrdered` | `number` |  |
| `quantityFulfilled` | `number` |  |
| `quantityRemaining` | `number` |  |
| `status` | `BackorderStatus` | Types: [`BackorderStatus`](#backorderstatus) |
| `priority` | `BackorderPriority` | Types: [`BackorderPriority`](#backorderpriority) |
| `createdAt` | `string` |  |

### BackorderPriority

Backorder priority as rendered on `BackorderOutput.priority` (Rust `Debug` form).

```ts
type BackorderPriority = 'Low' | 'Normal' | 'High' | 'Critical'
```

One of: `'Low'`, `'Normal'`, `'High'`, `'Critical'`.

### BackorderPriorityFilter

Backorder priority accepted by `BackorderFilterInput.priority`: the rendered form or lowercase (strict).

```ts
type BackorderPriorityFilter = BackorderPriority | BackorderPriorityInput
```

Types: [`BackorderPriority`](#backorderpriority), [`BackorderPriorityInput`](#backorderpriorityinput)

### BackorderPriorityInput

Backorder priority accepted by `CreateBackorderInput.priority` (case-insensitive).

```ts
type BackorderPriorityInput = 'critical' | 'high' | 'normal' | 'low'
```

One of: `'critical'`, `'high'`, `'normal'`, `'low'`.

### BackorderStatus

Backorder status as rendered on `BackorderOutput.status` (Rust `Debug` form).

```ts
type BackorderStatus = 'Pending' | 'PartiallyFulfilled' | 'Allocated' | 'ReadyToShip' | 'Fulfilled' | 'Cancelled'
```

One of: `'Pending'`, `'PartiallyFulfilled'`, `'Allocated'`, `'ReadyToShip'`, `'Fulfilled'`, `'Cancelled'`.

### BackorderStatusInput

Backorder status accepted by `BackorderFilterInput.status`: the rendered form or the engine's snake_case (strict).

```ts
type BackorderStatusInput = BackorderStatus | 'pending' | 'partially_fulfilled' | 'allocated' | 'ready_to_ship' | 'fulfilled' | 'cancelled' | 'canceled'
```

Types: [`BackorderStatus`](#backorderstatus)

### BackorderSummaryOutput

| Field | Type | Description |
|---|---|---|
| `totalBackorders` | `number` |  |
| `criticalCount` | `number` |  |
| `overdueCount` | `number` |  |
| `totalValue` | `number` | Total units on backorder, not a currency amount: this wraps `BackorderSummary::total_quantity`. The name is a historical misnomer kept for compatibility, which is why it carries no exact-money twin. |

### BackupManifestOutput

Manifest written alongside a database backup.

| Field | Type | Description |
|---|---|---|
| `manifestVersion` | `number` |  |
| `schemaVersion` | `string` |  |
| `migrationCount` | `number` |  |
| `engineVersion` | `string` |  |
| `createdAt` | `string` |  |
| `sourcePath` | `string` |  |
| `sizeBytes` | `number` |  |
| `checksum` | `string` |  |

### BackupReportOutput

Result of a database backup.

| Field | Type | Description |
|---|---|---|
| `backupPath` | `string` |  |
| `manifestPath` | `string` |  |
| `manifest` | `BackupManifestOutput` | Types: [`BackupManifestOutput`](#backupmanifestoutput) |

### BalanceSheetOutput

| Field | Type | Description |
|---|---|---|
| `asOfDate` | `string` |  |
| `totalAssets` | `number` | **Deprecated.** Use the `totalAssetsExact` twin; float money will be removed in 2.0. |
| `totalAssetsExact` | `string` | Exact base-10 total assets, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `totalLiabilities` | `number` | **Deprecated.** Use the `totalLiabilitiesExact` twin; float money will be removed in 2.0. |
| `totalLiabilitiesExact` | `string` | Exact base-10 total liabilities, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `totalEquity` | `number` | **Deprecated.** Use the `totalEquityExact` twin; float money will be removed in 2.0. |
| `totalEquityExact` | `string` | Exact base-10 total equity, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### BillFilterInput

Optional filters for `AccountsPayable.listBills`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `supplierId?` | `string` |  |
| `status?` | `BillStatusInput` | The rendered form (`PartiallyPaid`) or the engine's snake_case (`partially_paid`). Types: [`BillStatusInput`](#billstatusinput) |
| `purchaseOrderId?` | `string` |  |
| `overdueOnly?` | `boolean` |  |
| `fromDate?` | `string` | RFC 3339 timestamp. |
| `toDate?` | `string` | RFC 3339 timestamp. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### BillOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `billNumber` | `string` |  |
| `supplierId` | `string` |  |
| `status` | `BillStatus` | Types: [`BillStatus`](#billstatus) |
| `totalAmount` | `number` | **Deprecated.** Use the `totalAmountExact` twin; float money will be removed in 2.0. |
| `totalAmountExact` | `string` | Exact base-10 total amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `amountPaid` | `number` | **Deprecated.** Use the `amountPaidExact` twin; float money will be removed in 2.0. |
| `amountPaidExact` | `string` | Exact base-10 amount paid, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `amountDue` | `number` | **Deprecated.** Use the `amountDueExact` twin; float money will be removed in 2.0. |
| `amountDueExact` | `string` | Exact base-10 amount due, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `dueDate` | `string` |  |
| `createdAt` | `string` |  |

### BillStatus

Bill status as rendered on `BillOutput.status` (Rust `Debug` form).

```ts
type BillStatus = 'Draft' | 'Pending' | 'Approved' | 'PartiallyPaid' | 'Paid' | 'Overdue' | 'Cancelled' | 'Disputed'
```

One of: `'Draft'`, `'Pending'`, `'Approved'`, `'PartiallyPaid'`, `'Paid'`, `'Overdue'`, `'Cancelled'`, `'Disputed'`.

### BillStatusInput

Bill status accepted by `BillFilterInput.status`: the rendered form or the engine's snake_case (strict).

```ts
type BillStatusInput = BillStatus | 'draft' | 'pending' | 'approved' | 'partially_paid' | 'paid' | 'overdue' | 'cancelled' | 'canceled' | 'disputed'
```

Types: [`BillStatus`](#billstatus)

### BillingCycleFilterInput

| Field | Type | Description |
|---|---|---|
| `subscriptionId?` | `string` |  |
| `status?` | `BillingCycleStatus` | Types: [`BillingCycleStatus`](#billingcyclestatus) |
| `fromDate?` | `string` |  |
| `toDate?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### BillingCycleOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `subscriptionId` | `string` |  |
| `cycleNumber` | `number` |  |
| `status` | `BillingCycleStatus` | Types: [`BillingCycleStatus`](#billingcyclestatus) |
| `periodStart` | `string` |  |
| `periodEnd` | `string` |  |
| `subtotal` | `number` | **Deprecated.** Use the `subtotalExact` twin; float money will be removed in 2.0. |
| `subtotalExact` | `string` | Exact base-10 subtotal, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `discount` | `number` | **Deprecated.** Use the `discountExact` twin; float money will be removed in 2.0. |
| `discountExact` | `string` | Exact base-10 discount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `tax` | `number` | **Deprecated.** Use the `taxExact` twin; float money will be removed in 2.0. |
| `taxExact` | `string` | Exact base-10 tax, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `total` | `number` | **Deprecated.** Use the `totalExact` twin; float money will be removed in 2.0. |
| `totalExact` | `string` | Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `currency` | `string` |  |
| `paymentId?` | `string` |  |
| `billedAt?` | `string` |  |
| `failureReason?` | `string` |  |
| `retryCount` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### BillingCycleStatus

Billing cycle status, shared by `BillingCycleOutput.status` and `BillingCycleFilterInput.status` (case-insensitive on input).

```ts
type BillingCycleStatus = 'scheduled' | 'processing' | 'paid' | 'failed' | 'skipped' | 'refunded' | 'voided'
```

One of: `'scheduled'`, `'processing'`, `'paid'`, `'failed'`, `'skipped'`, `'refunded'`, `'voided'`.

### BomComponentOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `bomId` | `string` |  |
| `componentSku?` | `string` |  |
| `name` | `string` |  |
| `quantity` | `number` |  |
| `unitOfMeasure` | `string` |  |

### BomFilterInput

Filter for `bom.list()`

| Field | Type | Description |
|---|---|---|
| `productId?` | `string` |  |
| `status?` | `BomStatus` | BOM status Types: [`BomStatus`](#bomstatus) |
| `search?` | `string` | Free-text search over name and BOM number |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |

### BomOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `bomNumber` | `string` |  |
| `name` | `string` |  |
| `productId` | `string` |  |
| `status` | `BomStatus` | Types: [`BomStatus`](#bomstatus) |
| `revision` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### BomStatus

BOM status (`BomOutput.status`, `BomFilterInput.status`).

```ts
type BomStatus = 'draft' | 'active' | 'obsolete'
```

One of: `'draft'`, `'active'`, `'obsolete'`.

### BoostRuleInput

| Field | Type | Description |
|---|---|---|
| `field` | `string` |  |
| `valueMatch` | `string` |  |
| `boostFactor` | `number` |  |

### BoostRuleOutput

| Field | Type | Description |
|---|---|---|
| `field` | `string` |  |
| `valueMatch` | `string` |  |
| `boostFactor` | `number` |  |

### BulkSupplierSkuItemInput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `sku` | `string` |  |
| `unitCost?` | `string` | Exact decimal string |

### CanadianTaxInfoOutput

| Field | Type | Description |
|---|---|---|
| `provinceCode` | `string` |  |
| `provinceName` | `string` |  |
| `gstRate` | `number` |  |
| `pstRate?` | `number` |  |
| `hstRate?` | `number` |  |
| `qstRate?` | `number` |  |
| `totalRate` | `number` |  |

### CancelSubscriptionInput

| Field | Type | Description |
|---|---|---|
| `reason?` | `string` |  |
| `immediate?` | `boolean` |  |
| `feedback?` | `string` |  |

### CaptureStockLineInput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `sku` | `string` |  |
| `quantityOnHand` | `string` | Exact decimal string |
| `quantityAvailable` | `string` | Exact decimal string |
| `location?` | `string` |  |

### CaptureStockSnapshotInput

| Field | Type | Description |
|---|---|---|
| `label?` | `string` |  |
| `lines` | `Array<CaptureStockLineInput>` | Types: [`CaptureStockLineInput`](#capturestocklineinput) |

### CaptureTopologySnapshotInput

| Field | Type | Description |
|---|---|---|
| `channelsTotal` | `string` |  |
| `channelsActive` | `string` |  |
| `warehousesTotal` | `string` |  |
| `productsTotal` | `string` |  |
| `openOrders` | `string` |  |
| `signals?` | `string` | JSON string |

### CartAddressInput

| Field | Type | Description |
|---|---|---|
| `firstName` | `string` |  |
| `lastName` | `string` |  |
| `company?` | `string` |  |
| `line1` | `string` |  |
| `line2?` | `string` |  |
| `city` | `string` |  |
| `state?` | `string` |  |
| `postalCode` | `string` |  |
| `country` | `string` |  |
| `phone?` | `string` |  |
| `email?` | `string` |  |

### CartAddressOutput

| Field | Type | Description |
|---|---|---|
| `firstName` | `string` |  |
| `lastName` | `string` |  |
| `company?` | `string` |  |
| `line1` | `string` |  |
| `line2?` | `string` |  |
| `city` | `string` |  |
| `state?` | `string` |  |
| `postalCode` | `string` |  |
| `country` | `string` |  |
| `phone?` | `string` |  |
| `email?` | `string` |  |

### CartAddressSnapshot

Cart address as the engine stores it (`CartAddress`).

| Field | Type | Description |
|---|---|---|
| `first_name` | `string` |  |
| `last_name` | `string` |  |
| `company` | `string \| null` |  |
| `line1` | `string` |  |
| `line2` | `string \| null` |  |
| `city` | `string` |  |
| `state` | `string \| null` |  |
| `postal_code` | `string` |  |
| `country` | `string` |  |
| `phone` | `string \| null` |  |
| `email` | `string \| null` |  |

### CartFilterInput

Filter for `carts.list()`

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `customerEmail?` | `string` |  |
| `status?` | `CartStatus \| 'readyforpayment' \| 'paymentpending' \| 'canceled'` | Cart status (`canceled` and the unseparated spellings are accepted too) Types: [`CartStatus`](#cartstatus) |
| `hasItems?` | `boolean` |  |
| `isAbandoned?` | `boolean` |  |
| `createdAfter?` | `string` | RFC 3339 timestamp (inclusive lower bound on created_at) |
| `createdBefore?` | `string` | RFC 3339 timestamp (inclusive upper bound on created_at) |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |

### CartItemOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `cartId` | `string` |  |
| `productId?` | `string` |  |
| `variantId?` | `string` |  |
| `sku` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `imageUrl?` | `string` |  |
| `quantity` | `number` |  |
| `unitPrice` | `number` | **Deprecated.** Use the `unitPriceExact` twin; float money will be removed in 2.0. |
| `unitPriceExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `originalPrice?` | `number` | **Deprecated.** Use the `originalPriceExact` twin; float money will be removed in 2.0. |
| `originalPriceExact?` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `discountAmount` | `number` | **Deprecated.** Use the `discountAmountExact` twin; float money will be removed in 2.0. |
| `discountAmountExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `taxAmount` | `number` | **Deprecated.** Use the `taxAmountExact` twin; float money will be removed in 2.0. |
| `taxAmountExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `total` | `number` | **Deprecated.** Use the `totalExact` twin; float money will be removed in 2.0. |
| `totalExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `requiresShipping` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### CartItemSnapshot

Cart line as the engine stores it (`CartItem`).

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `cart_id` | `string` |  |
| `product_id` | `string \| null` |  |
| `variant_id` | `string \| null` |  |
| `sku` | `string` |  |
| `name` | `string` |  |
| `description` | `string \| null` |  |
| `image_url` | `string \| null` |  |
| `quantity` | `number` |  |
| `unit_price` | `string` |  |
| `original_price` | `string \| null` |  |
| `discount_amount` | `string` |  |
| `tax_amount` | `string` |  |
| `total` | `string` |  |
| `weight` | `string \| null` |  |
| `requires_shipping` | `boolean` |  |
| `metadata` | `Record<string, unknown> \| null` |  |
| `created_at` | `string` |  |
| `updated_at` | `string` |  |

### CartOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `cartNumber` | `string` |  |
| `customerId?` | `string` |  |
| `status` | `CartStatus` | Types: [`CartStatus`](#cartstatus) |
| `currency` | `string` |  |
| `subtotal` | `number` | **Deprecated.** Use the `subtotalExact` twin; float money will be removed in 2.0. |
| `subtotalExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `taxAmount` | `number` | **Deprecated.** Use the `taxAmountExact` twin; float money will be removed in 2.0. |
| `taxAmountExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `shippingAmount` | `number` | **Deprecated.** Use the `shippingAmountExact` twin; float money will be removed in 2.0. |
| `shippingAmountExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `discountAmount` | `number` | **Deprecated.** Use the `discountAmountExact` twin; float money will be removed in 2.0. |
| `discountAmountExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `grandTotal` | `number` | **Deprecated.** Use the `grandTotalExact` twin; float money will be removed in 2.0. |
| `grandTotalExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `customerEmail?` | `string` |  |
| `customerPhone?` | `string` |  |
| `customerName?` | `string` |  |
| `shippingAddress?` | `CartAddressOutput` | Types: [`CartAddressOutput`](#cartaddressoutput) |
| `billingAddress?` | `CartAddressOutput` | Types: [`CartAddressOutput`](#cartaddressoutput) |
| `billingSameAsShipping` | `boolean` |  |
| `fulfillmentType?` | `FulfillmentType` | Types: [`FulfillmentType`](#fulfillmenttype) |
| `shippingMethod?` | `string` |  |
| `shippingCarrier?` | `string` |  |
| `paymentMethod?` | `string` |  |
| `paymentStatus` | `CartPaymentStatus` | Types: [`CartPaymentStatus`](#cartpaymentstatus) |
| `couponCode?` | `string` |  |
| `orderId?` | `string` |  |
| `orderNumber?` | `string` |  |
| `inventoryReserved` | `boolean` |  |
| `itemCount` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### CartPaymentStatus

Cart-level payment progress (`CartOutput.paymentStatus`).

```ts
type CartPaymentStatus = 'none' | 'method_selected' | 'authorized' | 'captured' | 'failed' | 'refunded'
```

One of: `'none'`, `'method_selected'`, `'authorized'`, `'captured'`, `'failed'`, `'refunded'`.

### CartSnapshot

The cart exactly as the fingerprint was computed over it
(`stateset_core::Cart`, snake_case, exact decimals). This is the engine's
own aggregate, not the camelCase `CartOutput` the `carts` API returns.

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `cart_number` | `string` |  |
| `customer_id` | `string \| null` |  |
| `status` | `'active' \| 'ready_for_payment' \| 'payment_pending' \| 'completed' \| 'abandoned' \| 'cancelled' \| 'expired'` |  |
| `currency` | `string` |  |
| `items` | `CartItemSnapshot[]` | Types: [`CartItemSnapshot`](#cartitemsnapshot) |
| `subtotal` | `string` |  |
| `tax_amount` | `string` |  |
| `shipping_amount` | `string` |  |
| `discount_amount` | `string` |  |
| `grand_total` | `string` |  |
| `customer_email` | `string \| null` |  |
| `customer_phone` | `string \| null` |  |
| `customer_name` | `string \| null` |  |
| `shipping_address` | `CartAddressSnapshot \| null` | Types: [`CartAddressSnapshot`](#cartaddresssnapshot) |
| `billing_address` | `CartAddressSnapshot \| null` | Types: [`CartAddressSnapshot`](#cartaddresssnapshot) |
| `billing_same_as_shipping` | `boolean` |  |
| `fulfillment_type` | `'shipping' \| 'pickup' \| 'digital' \| null` |  |
| `shipping_method` | `string \| null` |  |
| `shipping_carrier` | `string \| null` |  |
| `estimated_delivery` | `string \| null` |  |
| `payment_method` | `string \| null` |  |
| `payment_token` | `string \| null` |  |
| `payment_status` | `'none' \| 'method_selected' \| 'authorized' \| 'captured' \| 'failed' \| 'refunded'` |  |
| `coupon_code` | `string \| null` |  |
| `discount_description` | `string \| null` |  |
| `order_id` | `string \| null` |  |
| `order_number` | `string \| null` |  |
| `notes` | `string \| null` |  |
| `metadata` | `Record<string, unknown> \| null` |  |
| `inventory_reserved` | `boolean` |  |
| `reservation_expires_at` | `string \| null` |  |
| `x402_payment` | `CartX402PaymentSnapshot \| null` | Types: [`CartX402PaymentSnapshot`](#cartx402paymentsnapshot) |
| `expires_at` | `string \| null` |  |
| `completed_at` | `string \| null` |  |
| `created_at` | `string` |  |
| `updated_at` | `string` |  |

### CartStatus

Cart lifecycle status (`CartOutput.status`, `CartFilterInput.status`).

```ts
type CartStatus = 'active' | 'ready_for_payment' | 'payment_pending' | 'completed' | 'abandoned' | 'cancelled' | 'expired'
```

One of: `'active'`, `'ready_for_payment'`, `'payment_pending'`, `'completed'`, `'abandoned'`, `'cancelled'`, `'expired'`.

### CartX402PaymentSnapshot

x402 stablecoin payment attached to a cart (`CartX402Payment`).

| Field | Type | Description |
|---|---|---|
| `intent_id` | `string \| null` |  |
| `payer_address` | `string` |  |
| `network` | `'set_chain' \| 'set_chain_testnet' \| 'base' \| 'arc' \| 'arc_testnet' \| 'base_sepolia' \| 'ethereum' \| 'ethereum_sepolia' \| 'arbitrum' \| 'optimism'` |  |
| `asset` | `'usdc' \| 'usdt' \| 'ssusd' \| 'wssusd' \| 'dai' \| 'eth'` |  |
| `status` | `'created' \| 'signed' \| 'sequenced' \| 'batched' \| 'settled' \| 'expired' \| 'failed' \| 'cancelled'` |  |

### ChannelFilterInput

| Field | Type | Description |
|---|---|---|
| `channelType?` | `ChannelType` | sales_channel, fulfillment_channel, end_to_end_channel Types: [`ChannelType`](#channeltype) |
| `status?` | `ChannelStatus` | active, paused, deleted Types: [`ChannelStatus`](#channelstatus) |
| `integration?` | `string` |  |
| `apiLocked?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### ChannelOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `channelType` | `ChannelType` | sales_channel, fulfillment_channel, end_to_end_channel Types: [`ChannelType`](#channeltype) |
| `integration?` | `string` |  |
| `status` | `ChannelStatus` | active, paused, deleted Types: [`ChannelStatus`](#channelstatus) |
| `apiLocked` | `boolean` |  |
| `defaultWarehouseId?` | `string` |  |
| `tags` | `Array<string>` |  |
| `metadata` | `string` | Metadata as JSON |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ChannelProductMappingOutput

| Field | Type | Description |
|---|---|---|
| `channelId` | `string` |  |
| `channelSku` | `string` |  |
| `productId` | `string` |  |
| `internalSku` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ChannelProductSyncItemInput

| Field | Type | Description |
|---|---|---|
| `channelSku` | `string` |  |
| `productId?` | `string` |  |
| `internalSku?` | `string` |  |
| `delete?` | `boolean` | When true, remove the mapping instead of upserting it |

### ChannelStatus

Channel lifecycle status (`ChannelStatus`).

```ts
type ChannelStatus = 'active' | 'paused' | 'deleted'
```

One of: `'active'`, `'paused'`, `'deleted'`.

### ChannelType

Direction of order flow through a channel (`ChannelType`); fixed once set.

```ts
type ChannelType = 'sales_channel' | 'fulfillment_channel' | 'end_to_end_channel'
```

One of: `'sales_channel'`, `'fulfillment_channel'`, `'end_to_end_channel'`.

### CheckoutResultOutput

| Field | Type | Description |
|---|---|---|
| `cartId` | `string` |  |
| `orderId` | `string` |  |
| `orderNumber` | `string` |  |
| `paymentId?` | `string` |  |
| `totalCharged` | `number` | **Deprecated.** Use the `totalChargedExact` twin; float money will be removed in 2.0. |
| `totalChargedExact` | `string` | _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `currency` | `string` |  |

### CheckoutSnapshot

Exact quote terms and their fingerprint, read from one cart snapshot
(`Commerce.checkoutSnapshot`). Keep the result with the issued quote and
pass `fingerprint` as `expected_cart_fingerprint` on `checkout.commit`;
never recalculate it at acceptance.

| Field | Type | Description |
|---|---|---|
| `cart` | `CartSnapshot` | Types: [`CartSnapshot`](#cartsnapshot) |
| `fingerprint` | `string` | `"sha256:<hex>"` over the canonical (JCS) cart snapshot, version-tagged. |

### CloseMonthOptionsInput

| Field | Type | Description |
|---|---|---|
| `dryRun?` | `boolean` | Compute per-step counts/amounts without writing anything |
| `skipDepreciation?` | `boolean` | Skip posting scheduled fixed-asset depreciation |
| `skipRevenueRecognition?` | `boolean` | Skip recognizing deferred revenue through period end |
| `skipFxRevaluation?` | `boolean` | Skip FX revaluation of foreign-currency accounts |
| `skipPeriodClose?` | `boolean` | Skip the final period close (closing entries + close period) |
| `closedBy?` | `string` | Actor recorded as the closer; defaults to `system` |

### CloseMonthReportOutput

| Field | Type | Description |
|---|---|---|
| `periodId` | `string` |  |
| `periodName` | `string` |  |
| `dryRun` | `boolean` |  |
| `depreciation` | `CloseMonthStepOutput` | Step 1: scheduled depreciation due through period end Types: [`CloseMonthStepOutput`](#closemonthstepoutput) |
| `revenueRecognition` | `CloseMonthStepOutput` | Step 2: deferred revenue recognized through period end Types: [`CloseMonthStepOutput`](#closemonthstepoutput) |
| `fxRevaluation` | `CloseMonthStepOutput` | Step 3: FX revaluation as of period end Types: [`CloseMonthStepOutput`](#closemonthstepoutput) |
| `periodClose` | `CloseMonthStepOutput` | Step 4: closing entries + close period Types: [`CloseMonthStepOutput`](#closemonthstepoutput) |
| `closingEntry?` | `JournalEntryOutput` | Posted closing entry; None for dry runs or skipped closes Types: [`JournalEntryOutput`](#journalentryoutput) |
| `periodStatus` | `GlPeriodStatus` | Period status after the run (`closed` after a real close) Types: [`GlPeriodStatus`](#glperiodstatus) |

### CloseMonthStepOutput

| Field | Type | Description |
|---|---|---|
| `status` | `CloseMonthStepStatus` | One of `executed`, `skipped`, `dry_run` Types: [`CloseMonthStepStatus`](#closemonthstepstatus) |
| `entryCount` | `number` | Entries posted (or that would be posted in a dry run) |
| `totalAmount` | `string` | Exact decimal string |
| `warnings` | `Array<string>` | Per-item failures that did not abort the close |

### CloseMonthStepStatus

Outcome of one month-end close step on `CloseMonthStepOutput.status`.

```ts
type CloseMonthStepStatus = 'executed' | 'skipped' | 'dry_run'
```

One of: `'executed'`, `'skipped'`, `'dry_run'`.

### CommerceEvent

A commerce event as delivered to a subscriber: the serialised
`CommerceEvent` (tagged by `type`) plus a stable snake_case `event_type`.

| Field | Type | Description |
|---|---|---|
| `event_type` | `string` |  |
| `type?` | `string` |  |
| `[key: string]` | `unknown` |  |

### CompanyFilterInput

| Field | Type | Description |
|---|---|---|
| `status?` | `CompanyStatus` | active, inactive Types: [`CompanyStatus`](#companystatus) |
| `search?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### CompanyOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `reference?` | `string` |  |
| `email?` | `string` |  |
| `phone?` | `string` |  |
| `currency` | `string` |  |
| `paymentTermsDays?` | `number` |  |
| `status` | `CompanyStatus` | active, inactive Types: [`CompanyStatus`](#companystatus) |
| `tags` | `Array<string>` |  |
| `metadata` | `string` | Metadata as JSON |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### CompanyPriceOverrideOutput

| Field | Type | Description |
|---|---|---|
| `companyId` | `string` |  |
| `productId` | `string` |  |
| `price` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### CompanyShippingAddressOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `companyId` | `string` |  |
| `label?` | `string` |  |
| `name?` | `string` |  |
| `line1` | `string` |  |
| `line2?` | `string` |  |
| `city` | `string` |  |
| `region?` | `string` |  |
| `postalCode?` | `string` |  |
| `country` | `string` |  |
| `isDefault` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### CompanyStatus

B2B company account status (`CompanyStatus`).

```ts
type CompanyStatus = 'active' | 'inactive'
```

One of: `'active'`, `'inactive'`.

### ContactOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `firstName` | `string` |  |
| `lastName?` | `string` |  |
| `email?` | `string` |  |
| `phone?` | `string` |  |
| `title?` | `string` |  |
| `companyIds` | `Array<string>` |  |
| `portalEnabled` | `boolean` |  |
| `isActive` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ConversionResultOutput

| Field | Type | Description |
|---|---|---|
| `originalAmount` | `number` | **Deprecated.** Use the `originalAmountExact` twin; float money will be removed in 2.0. |
| `originalAmountExact` | `string` | Exact base-10 original amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `originalCurrency` | `string` |  |
| `convertedAmount` | `number` | **Deprecated.** Use the `convertedAmountExact` twin; float money will be removed in 2.0. |
| `convertedAmountExact` | `string` | Exact base-10 converted amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `targetCurrency` | `string` |  |
| `rate` | `number` |  |
| `inverseRate` | `number` |  |
| `rateAt` | `string` |  |

### ConversionRuleType

Scope of a unit conversion rule (`ConversionRuleType`); rendered UPPERCASE, parsed case-insensitively.

```ts
type ConversionRuleType = 'SYSTEM' | 'SKU'
```

One of: `'SYSTEM'`, `'SKU'`.

### ConvertCurrencyInput

| Field | Type | Description |
|---|---|---|
| `from` | `string` | Source currency code |
| `to` | `string` | Target currency code |
| `amount` | `number` | Amount to convert |

### CostMethod

Inventory costing method as rendered on `ItemCostOutput.costMethod` (Rust `Debug` form).

```ts
type CostMethod = 'Average' | 'Fifo' | 'Lifo' | 'Standard' | 'Specific'
```

One of: `'Average'`, `'Fifo'`, `'Lifo'`, `'Standard'`, `'Specific'`.

### CostMethodFilter

Costing method accepted by `ItemCostFilterInput.costMethod`: the rendered form or the engine's snake_case (strict).

```ts
type CostMethodFilter = CostMethod | 'average' | 'avg' | 'fifo' | 'lifo' | 'standard' | 'std' | 'specific'
```

Types: [`CostMethod`](#costmethod)

### CostMethodInput

Costing method accepted by `SetItemCostInput.costMethod` (case-insensitive).

```ts
type CostMethodInput = 'standard' | 'average' | 'fifo' | 'lifo'
```

One of: `'standard'`, `'average'`, `'fifo'`, `'lifo'`.

### CouponFilterInput

Filter for listing coupons

| Field | Type | Description |
|---|---|---|
| `promotionId?` | `string` |  |
| `status?` | `CouponStatus` | Types: [`CouponStatus`](#couponstatus) |
| `search?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### CouponOutput

Coupon code output

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `promotionId` | `string` |  |
| `code` | `string` |  |
| `status` | `CouponStatus` | Types: [`CouponStatus`](#couponstatus) |
| `usageLimit?` | `number` |  |
| `perCustomerLimit?` | `number` |  |
| `usageCount` | `number` |  |
| `startsAt?` | `string` |  |
| `endsAt?` | `string` |  |
| `metadata?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### CouponStatus

Coupon status, shared by `CouponOutput.status` and `CouponFilterInput.status` (case-insensitive on input).

```ts
type CouponStatus = 'active' | 'disabled' | 'exhausted' | 'expired'
```

One of: `'active'`, `'disabled'`, `'exhausted'`, `'expired'`.

### CreateAgentFeedbackInput

| Field | Type | Description |
|---|---|---|
| `agentRegistry` | `string` |  |
| `agentId` | `string` |  |
| `clientAddress` | `string` |  |
| `value` | `string` | Signed integer value as a decimal string |
| `valueDecimals` | `number` | Number of decimal places encoded in `value` |
| `tag1?` | `string` |  |
| `tag2?` | `string` |  |
| `endpoint?` | `string` |  |
| `feedbackUri?` | `string` |  |
| `feedbackHash?` | `string` |  |

### CreateAgentIdentityInput

| Field | Type | Description |
|---|---|---|
| `agentRegistry` | `string` |  |
| `agentId` | `string` |  |
| `agentUri` | `string` |  |
| `agentWallet?` | `string` |  |
| `ownerAddress?` | `string` |  |
| `agentCardId?` | `string` |  |
| `registration?` | `string` | JSON-encoded registration document |
| `registrationHash?` | `string` |  |
| `walletProofType?` | `AgentWalletProofTypeInput` | Snake-case proof type Types: [`AgentWalletProofTypeInput`](#agentwalletprooftypeinput) |
| `walletProof?` | `string` |  |
| `walletProofChainId?` | `string` | Chain id as a decimal string |
| `walletProofDeadline?` | `string` | RFC3339 timestamp |
| `active?` | `boolean` |  |

### CreateAgentValidationRequestInput

| Field | Type | Description |
|---|---|---|
| `requestHash` | `string` |  |
| `agentRegistry` | `string` |  |
| `agentId` | `string` |  |
| `validatorAddress` | `string` |  |
| `requestUri` | `string` |  |

### CreateAgentValidationResponseInput

| Field | Type | Description |
|---|---|---|
| `response` | `number` | Validation score (0-100) |
| `responseUri?` | `string` |  |
| `responseHash?` | `string` |  |
| `tag?` | `string` |  |

### CreateBackorderInput

| Field | Type | Description |
|---|---|---|
| `orderId` | `string` |  |
| `customerId` | `string` |  |
| `sku` | `string` |  |
| `quantity` | `number` |  |
| `priority?` | `BackorderPriorityInput` | Types: [`BackorderPriorityInput`](#backorderpriorityinput) |
| `notes?` | `string` |  |

### CreateBillInput

| Field | Type | Description |
|---|---|---|
| `supplierId` | `string` |  |
| `dueDate` | `string` |  |
| `paymentTerms?` | `string` |  |
| `referenceNumber?` | `string` |  |
| `notes?` | `string` |  |

### CreateBomComponentInput

| Field | Type | Description |
|---|---|---|
| `componentSku?` | `string` |  |
| `name` | `string` |  |
| `quantity` | `number` |  |
| `unitOfMeasure?` | `string` |  |

### CreateBomInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `productId` | `string` |  |
| `description?` | `string` |  |
| `revision?` | `string` |  |

### CreateCartInput

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `customerEmail?` | `string` |  |
| `customerName?` | `string` |  |
| `currency?` | `string` |  |
| `shippingAddress?` | `CartAddressInput` | Types: [`CartAddressInput`](#cartaddressinput) |
| `billingAddress?` | `CartAddressInput` | Types: [`CartAddressInput`](#cartaddressinput) |
| `notes?` | `string` |  |
| `expiresInMinutes?` | `number` |  |

### CreateChannelInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `channelType` | `ChannelType` | sales_channel, fulfillment_channel, end_to_end_channel Types: [`ChannelType`](#channeltype) |
| `integration?` | `string` |  |
| `defaultWarehouseId?` | `string` |  |
| `tags?` | `Array<string>` |  |
| `metadata?` | `string` | Metadata as JSON |

### CreateCompanyInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `reference?` | `string` |  |
| `email?` | `string` |  |
| `phone?` | `string` |  |
| `currency?` | `string` | ISO 4217 currency code |
| `paymentTermsDays?` | `number` |  |
| `tags?` | `Array<string>` |  |
| `metadata?` | `string` | Metadata as JSON |

### CreateContactInput

| Field | Type | Description |
|---|---|---|
| `firstName` | `string` |  |
| `lastName?` | `string` |  |
| `email?` | `string` |  |
| `phone?` | `string` |  |
| `title?` | `string` |  |
| `companyIds?` | `Array<string>` |  |

### CreateCouponInput

Input for creating a coupon code

| Field | Type | Description |
|---|---|---|
| `promotionId` | `string` | Promotion ID this coupon is for |
| `code` | `string` | The coupon code customers enter |
| `usageLimit?` | `number` | Usage limit for this coupon |
| `perCustomerLimit?` | `number` | Per customer limit |
| `startsAt?` | `string` | Start date (RFC3339) |
| `endsAt?` | `string` | End date (RFC3339) |
| `metadata?` | `string` | Metadata as JSON |

### CreateCreditAccountInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `creditLimit` | `number` |  |
| `paymentTerms?` | `string` |  |
| `notes?` | `string` |  |

### CreateCreditMemoInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `originalInvoiceId?` | `string` |  |
| `reason` | `CreditMemoReasonInput` | Types: [`CreditMemoReasonInput`](#creditmemoreasoninput) |
| `amount` | `number` |  |
| `notes?` | `string` |  |

### CreateCustomObjectInput

| Field | Type | Description |
|---|---|---|
| `typeHandle` | `string` |  |
| `handle?` | `string` |  |
| `ownerType?` | `string` |  |
| `ownerId?` | `string` |  |
| `valuesJson` | `string` | JSON string representing record values (must be an object). |

### CreateCustomObjectTypeInput

| Field | Type | Description |
|---|---|---|
| `handle` | `string` |  |
| `displayName` | `string` |  |
| `description?` | `string` |  |
| `fields` | `Array<CustomFieldDefinitionInput>` | Types: [`CustomFieldDefinitionInput`](#customfielddefinitioninput) |

### CreateCustomerAddressInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `addressType?` | `AddressType` | Defaults to `both`. Types: [`AddressType`](#addresstype) |
| `firstName` | `string` |  |
| `lastName` | `string` |  |
| `company?` | `string` |  |
| `line1` | `string` |  |
| `line2?` | `string` |  |
| `city` | `string` |  |
| `state?` | `string` |  |
| `postalCode` | `string` |  |
| `country` | `string` |  |
| `phone?` | `string` |  |
| `isDefault?` | `boolean` |  |

### CreateCustomerInput

| Field | Type | Description |
|---|---|---|
| `email` | `string` |  |
| `firstName` | `string` |  |
| `lastName` | `string` |  |
| `phone?` | `string` |  |
| `acceptsMarketing?` | `boolean` |  |
| `tags?` | `Array<string>` |  |
| `metadata?` | `CustomerMetadata` | Types: [`CustomerMetadata`](#customermetadata) |

### CreateCycleCountInput

| Field | Type | Description |
|---|---|---|
| `warehouseId` | `number` |  |
| `locationId?` | `number` | Optional single location scope; omit to count across the warehouse. |
| `scheduledDate?` | `string` | RFC 3339 timestamp |
| `countedBy?` | `string` |  |
| `lines` | `Array<CreateCycleCountLineInput>` | Types: [`CreateCycleCountLineInput`](#createcyclecountlineinput) |

### CreateCycleCountLineInput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `lotId?` | `string` |  |
| `expectedQuantity` | `string` | Exact decimal string |

### CreateEdiDocumentInput

| Field | Type | Description |
|---|---|---|
| `documentType` | `string` | EDI document type (e.g. `850`, `855`, `856`, `810`) |
| `direction?` | `EdiDirection` | One of `inbound`, `outbound` (defaults to `inbound`) Types: [`EdiDirection`](#edidirection) |
| `partner?` | `string` | Trading partner name / id |
| `reference?` | `string` | Related business reference (PO number, order number, etc.) |
| `payload?` | `string` | Raw EDI payload |

### CreateExemptionInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `exemptionType` | `TaxExemptionTypeInput` | Types: [`TaxExemptionTypeInput`](#taxexemptiontypeinput) |
| `certificateNumber?` | `string` |  |
| `issuingAuthority?` | `string` |  |
| `jurisdictionIds?` | `Array<string>` |  |
| `exemptCategories?` | `ProductTaxCategory[]` | Types: [`ProductTaxCategory`](#producttaxcategory) |
| `effectiveFrom` | `string` |  |
| `expiresAt?` | `string` |  |
| `notes?` | `string` |  |

### CreateFixedAssetInput

| Field | Type | Description |
|---|---|---|
| `assetNumber?` | `string` | Optional asset number; auto-generated when omitted (FA-...) |
| `name` | `string` |  |
| `description?` | `string` |  |
| `category` | `FixedAssetCategory` | Category: land, building, machinery, equipment, vehicle, furniture_and_fixtures, computer_hardware, software, leasehold_improvement, other Types: [`FixedAssetCategory`](#fixedassetcategory) |
| `acquisitionDate` | `string` | ISO date (YYYY-MM-DD) |
| `acquisitionCost` | `string` | Exact decimal string, e.g. "10000.00" |
| `salvageValue` | `string` | Exact decimal string |
| `usefulLifeMonths` | `number` |  |
| `depreciationMethod` | `DepreciationMethod` | straight_line, declining_balance, units_of_production Types: [`DepreciationMethod`](#depreciationmethod) |
| `decliningBalanceRate?` | `string` | Required for declining_balance: periodic rate as exact decimal string strictly between 0 and 1 (e.g. "0.2" for 20%) |
| `inServiceDate?` | `string` | ISO date (YYYY-MM-DD) |
| `locationId?` | `string` |  |
| `assetAccountId?` | `string` |  |
| `accumulatedDepreciationAccountId?` | `string` |  |
| `depreciationExpenseAccountId?` | `string` |  |
| `currency?` | `string` | Currency code, e.g. "USD" |

### CreateFraudAssessmentInput

| Field | Type | Description |
|---|---|---|
| `orderId` | `string` |  |
| `signals` | `Array<CreateFraudSignalInput>` | Types: [`CreateFraudSignalInput`](#createfraudsignalinput) |

### CreateFraudRuleInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `description?` | `string` |  |
| `signalType` | `FraudSignalType` | Snake-case signal type Types: [`FraudSignalType`](#fraudsignaltype) |
| `threshold` | `number` |  |
| `action` | `FraudDecision` | Snake-case decision to apply when the rule triggers Types: [`FraudDecision`](#frauddecision) |

### CreateFraudSignalInput

| Field | Type | Description |
|---|---|---|
| `signalType` | `FraudSignalType` | Snake-case signal type: `velocity_spike`, `address_mismatch`, ... Types: [`FraudSignalType`](#fraudsignaltype) |
| `score` | `number` | Confidence score (0.0 - 1.0) |
| `details` | `string` |  |

### CreateGiftCardInput

| Field | Type | Description |
|---|---|---|
| `code?` | `string` | Redemption code (auto-generated if omitted) |
| `initialBalance` | `string` | Initial balance as an exact decimal string, e.g. "50.00" |
| `currency` | `string` | Currency code, e.g. "USD" |
| `recipientEmail?` | `string` |  |
| `senderName?` | `string` |  |
| `message?` | `string` |  |
| `expiresAt?` | `string` | RFC 3339 expiry timestamp |

### CreateGlAccountInput

| Field | Type | Description |
|---|---|---|
| `accountNumber` | `string` |  |
| `name` | `string` |  |
| `accountType` | `GlAccountTypeInput` | Types: [`GlAccountTypeInput`](#glaccounttypeinput) |
| `description?` | `string` |  |
| `currency?` | `string` |  |

### CreateGlPeriodInput

| Field | Type | Description |
|---|---|---|
| `periodName` | `string` | Display name, typically `YYYY-MM` |
| `fiscalYear` | `number` |  |
| `periodNumber` | `number` | Sequential number within the fiscal year (1-12 for monthly) |
| `startDate` | `string` | First date of the period (inclusive), ISO date (YYYY-MM-DD) |
| `endDate` | `string` | Last date of the period (inclusive), ISO date (YYYY-MM-DD) |

### CreateInboundShipmentInput

| Field | Type | Description |
|---|---|---|
| `supplierId` | `string` |  |
| `purchaseOrderId?` | `string` |  |
| `warehouseId?` | `string` |  |
| `carrier?` | `string` |  |
| `trackingNumber?` | `string` |  |
| `expectedAt?` | `string` | RFC 3339 timestamp |
| `items` | `Array<CreateInboundShipmentItemInput>` | Types: [`CreateInboundShipmentItemInput`](#createinboundshipmentiteminput) |
| `notes?` | `string` |  |

### CreateInboundShipmentItemInput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `sku` | `string` |  |
| `quantityExpected` | `string` | Exact decimal string |

### CreateInspectionInput

| Field | Type | Description |
|---|---|---|
| `inspectionType` | `InspectionTypeInput` | Types: [`InspectionTypeInput`](#inspectiontypeinput) |
| `referenceType` | `string` |  |
| `referenceId` | `string` |  |
| `warehouseId?` | `number` |  |
| `assignedTo?` | `string` |  |
| `notes?` | `string` |  |

### CreateIntegrationFieldMappingInput

| Field | Type | Description |
|---|---|---|
| `integrationAccount` | `string` |  |
| `mappingGroup` | `string` |  |
| `sourceField` | `string` |  |
| `destinationField` | `string` |  |
| `template?` | `string` |  |
| `transform?` | `FieldTransform` | Snake-case transform: `none`, `uppercase`, `lowercase`, `trim` Types: [`FieldTransform`](#fieldtransform) |
| `fallback?` | `string` |  |

### CreateIntegrationMappingInput

| Field | Type | Description |
|---|---|---|
| `integration` | `string` |  |
| `mappingGroup` | `string` |  |
| `fieldName` | `string` |  |
| `externalValue` | `string` |  |
| `internalValue` | `string` |  |

### CreateInventoryItemInput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `initialQuantity?` | `number` |  |
| `reorderPoint?` | `number` |  |

### CreateInvoiceInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `orderId?` | `string` |  |
| `items` | `Array<CreateInvoiceItemInput>` | Types: [`CreateInvoiceItemInput`](#createinvoiceiteminput) |
| `billingEmail?` | `string` |  |
| `billingName?` | `string` |  |
| `notes?` | `string` |  |

### CreateInvoiceItemInput

| Field | Type | Description |
|---|---|---|
| `description` | `string` |  |
| `quantity` | `number` |  |
| `unitPrice?` | `number` | Float unit price. Optional: send `unit_price_exact` instead for exact money. |
| `unitPriceExact?` | `string` | Exact base-10 unit price. Takes precedence over `unit_price` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `sku?` | `string` |  |

### CreateJurisdictionInput

| Field | Type | Description |
|---|---|---|
| `parentId?` | `string` |  |
| `name` | `string` |  |
| `code` | `string` |  |
| `level?` | `TaxJurisdictionLevel` | Types: [`TaxJurisdictionLevel`](#taxjurisdictionlevel) |
| `countryCode` | `string` |  |
| `stateCode?` | `string` |  |
| `county?` | `string` |  |
| `city?` | `string` |  |
| `postalCodes?` | `Array<string>` |  |

### CreateLocationInput

| Field | Type | Description |
|---|---|---|
| `warehouseId` | `number` |  |
| `locationType` | `WarehouseLocationTypeInput` | Types: [`WarehouseLocationTypeInput`](#warehouselocationtypeinput) |
| `zone?` | `string` |  |
| `aisle?` | `string` |  |
| `rack?` | `string` |  |
| `bin?` | `string` |  |
| `isPickable?` | `boolean` |  |
| `isReceivable?` | `boolean` |  |

### CreateLotInput

| Field | Type | Description |
|---|---|---|
| `lotNumber?` | `string` |  |
| `sku` | `string` |  |
| `quantityProduced` | `number` |  |
| `productionDate?` | `string` |  |
| `expirationDate?` | `string` |  |
| `supplierLotNumber?` | `string` |  |

### CreateLoyaltyProgramInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `description?` | `string` |  |
| `pointsPerDollar` | `number` |  |
| `tiers?` | `Array<LoyaltyTierInput>` | Types: [`LoyaltyTierInput`](#loyaltytierinput) |

### CreateNcrInput

| Field | Type | Description |
|---|---|---|
| `source` | `NcrSourceInput` | Types: [`NcrSourceInput`](#ncrsourceinput) |
| `severity` | `NcrSeverityInput` | Types: [`NcrSeverityInput`](#ncrseverityinput) |
| `sku` | `string` |  |
| `quantityAffected` | `number` |  |
| `description` | `string` |  |
| `lotNumber?` | `string` |  |
| `locationId?` | `number` |  |

### CreateOrderExactInput

Exact-money order input. Prefer this for financial and agent integrations.

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `cartId?` | `string` |  |
| `items` | `Array<CreateOrderItemExactInput>` | Types: [`CreateOrderItemExactInput`](#createorderitemexactinput) |
| `currency?` | `string` |  |
| `notes?` | `string` |  |
| `stockPolicy?` | `StockPolicy` | Defaults to `allow_backorder`. Types: [`StockPolicy`](#stockpolicy) |
| `shippingMethod?` | `string` |  |
| `shippingAddress?` | `OrderAddressInput` | Types: [`OrderAddressInput`](#orderaddressinput) |
| `billingAddress?` | `OrderAddressInput` | Types: [`OrderAddressInput`](#orderaddressinput) |

### CreateOrderInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `cartId?` | `string` |  |
| `items` | `Array<CreateOrderItemInput>` | Types: [`CreateOrderItemInput`](#createorderiteminput) |
| `currency?` | `string` |  |
| `notes?` | `string` |  |
| `stockPolicy?` | `StockPolicy` | Defaults to `allow_backorder`. Types: [`StockPolicy`](#stockpolicy) |
| `shippingMethod?` | `string` |  |
| `shippingAddress?` | `OrderAddressInput` | Types: [`OrderAddressInput`](#orderaddressinput) |
| `billingAddress?` | `OrderAddressInput` | Types: [`OrderAddressInput`](#orderaddressinput) |

### CreateOrderItemExactInput

Exact-money order item input. Monetary values are base-10 strings.

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name` | `string` |  |
| `quantity` | `number` |  |
| `unitPrice` | `string` |  |
| `taxAmount?` | `string` |  |
| `productId?` | `string` |  |
| `variantId?` | `string` |  |

### CreateOrderItemInput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name` | `string` |  |
| `quantity` | `number` |  |
| `unitPrice?` | `number` | Float unit price. Optional: send `unit_price_exact` instead for exact money. |
| `unitPriceExact?` | `string` | Exact base-10 unit price. Takes precedence over `unit_price` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `productId?` | `string` |  |
| `variantId?` | `string` |  |

### CreatePaymentExactInput

Exact-money payment input for agent and financial integrations.

| Field | Type | Description |
|---|---|---|
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `customerId?` | `string` |  |
| `idempotencyKey?` | `string` |  |
| `amount` | `string` |  |
| `currency?` | `string` |  |
| `paymentMethod?` | `PaymentMethodType` | Defaults to `credit_card`. Types: [`PaymentMethodType`](#paymentmethodtype) |

### CreatePaymentInput

| Field | Type | Description |
|---|---|---|
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `customerId?` | `string` |  |
| `idempotencyKey?` | `string` |  |
| `amount?` | `number` | Float amount. Optional: send `amount_exact` instead for exact money. |
| `amountExact?` | `string` | Exact base-10 amount. Takes precedence over `amount` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `currency?` | `string` |  |
| `paymentMethod?` | `PaymentMethodType` | Defaults to `credit_card`. Types: [`PaymentMethodType`](#paymentmethodtype) |

### CreatePaymentObligationInput

| Field | Type | Description |
|---|---|---|
| `supplierId` | `string` |  |
| `purchaseOrderId?` | `string` |  |
| `amount` | `string` | Exact decimal string |
| `currency?` | `string` |  |
| `dueDate` | `string` | Date string (YYYY-MM-DD) |
| `notes?` | `string` |  |

### CreatePerformanceObligationInput

| Field | Type | Description |
|---|---|---|
| `description` | `string` |  |
| `standaloneSellingPrice?` | `string` | Exact decimal string |
| `allocatedAmount` | `string` | Exact decimal string; obligations must sum to the transaction price |
| `recognitionMethod` | `RecognitionMethod` | point_in_time, ratable_over_time, milestone Types: [`RecognitionMethod`](#recognitionmethod) |
| `recognitionStart?` | `string` | ISO date (YYYY-MM-DD); required for ratable_over_time |
| `recognitionEnd?` | `string` | ISO date (YYYY-MM-DD); required for ratable_over_time |

### CreatePrepaymentInput

| Field | Type | Description |
|---|---|---|
| `supplierId` | `string` |  |
| `amount` | `string` | Exact decimal string, e.g. "1000.00" |
| `currency?` | `string` | Currency code, e.g. "USD" |
| `method?` | `string` | Payment method (e.g. "wire", "ach") |
| `reference?` | `string` |  |
| `memo?` | `string` |  |

### CreatePriceLevelInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `code` | `string` |  |
| `description?` | `string` |  |
| `adjustmentType?` | `PriceAdjustmentType` | none, percentage_discount, percentage_markup (default none) Types: [`PriceAdjustmentType`](#priceadjustmenttype) |
| `adjustmentValue?` | `string` | Percentage as exact decimal string (e.g. "10" for 10%); default "0" |
| `currency?` | `string` | Currency code, e.g. "USD" |

### CreatePriceScheduleInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `code?` | `string` |  |
| `currency?` | `string` | Currency code, e.g. "USD" |
| `startsAt?` | `string` | RFC 3339 timestamp |
| `endsAt?` | `string` | RFC 3339 timestamp |
| `priority?` | `number` | Priority used to break ties (higher wins); default 0 |

### CreatePrintStationInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `printers?` | `Array<string>` |  |

### CreateProductInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `slug?` | `string` |  |
| `description?` | `string` |  |
| `category?` | `string` |  |
| `variants?` | `Array<CreateProductVariantInput>` | Types: [`CreateProductVariantInput`](#createproductvariantinput) |

### CreateProductVariantInput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name?` | `string` |  |
| `price?` | `number` | Float price. Optional: send `price_exact` instead for exact money. |
| `priceExact?` | `string` | Exact base-10 price. Takes precedence over `price` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `compareAtPrice?` | `number` |  |
| `compareAtPriceExact?` | `string` | Exact base-10 comparison price. Takes precedence over `compare_at_price` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `isDefault?` | `boolean` |  |

### CreateProductionBatchInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `vendorId?` | `string` |  |
| `workOrderIds?` | `Array<string>` | Work order UUIDs to link at creation |
| `notes?` | `string` |  |
| `scheduledStart?` | `string` | RFC 3339 timestamp |
| `scheduledEnd?` | `string` | RFC 3339 timestamp |

### CreatePromotionInput

Input for creating a promotion

| Field | Type | Description |
|---|---|---|
| `code?` | `string` | Optional promotion code (auto-generated if not provided) |
| `name` | `string` | Display name |
| `description?` | `string` | Description for customers |
| `internalNotes?` | `string` | Internal notes |
| `promotionType?` | `PromotionTypeInput` | Type: percentage_off, fixed_amount_off, buy_x_get_y, free_shipping, tiered_discount, bundle Types: [`PromotionTypeInput`](#promotiontypeinput) |
| `trigger?` | `PromotionTriggerInput` | Trigger: automatic, coupon_code, both Types: [`PromotionTriggerInput`](#promotiontriggerinput) |
| `target?` | `PromotionTargetInput` | Target: order, product, category, shipping, line_item Types: [`PromotionTargetInput`](#promotiontargetinput) |
| `stacking?` | `PromotionStackingInput` | Stacking: stackable, exclusive, selective_stack Types: [`PromotionStackingInput`](#promotionstackinginput) |
| `percentageOff?` | `number` | Percentage off (0.0-1.0, e.g., 0.20 for 20%) |
| `fixedAmountOff?` | `number` | Fixed amount off |
| `maxDiscountAmount?` | `number` | Maximum discount amount (cap) |
| `buyQuantity?` | `number` | Buy X quantity (for BOGO) |
| `getQuantity?` | `number` | Get Y quantity (for BOGO) |
| `getDiscountPercent?` | `number` | Discount on "get" items (1.0 = free, 0.5 = 50% off) |
| `tiers?` | `string` | Tiered discount rules as JSON |
| `bundleProductIds?` | `Array<string>` | Bundle product IDs as JSON array |
| `bundleDiscount?` | `number` | Bundle discount |
| `startsAt?` | `string` | Start date (RFC3339) |
| `endsAt?` | `string` | End date (RFC3339) |
| `totalUsageLimit?` | `number` | Total usage limit |
| `perCustomerLimit?` | `number` | Per customer usage limit |
| `applicableProductIds?` | `Array<string>` | Applicable product IDs |
| `applicableCategoryIds?` | `Array<string>` | Applicable category IDs |
| `applicableSkus?` | `Array<string>` | Applicable SKUs |
| `excludedProductIds?` | `Array<string>` | Excluded product IDs |
| `excludedCategoryIds?` | `Array<string>` | Excluded category IDs |
| `eligibleCustomerIds?` | `Array<string>` | Eligible customer IDs |
| `eligibleCustomerGroups?` | `Array<string>` | Eligible customer groups |
| `currency?` | `string` | Currency code |
| `priority?` | `number` | Priority (lower = applied first) |
| `metadata?` | `string` | Metadata as JSON |

### CreatePurchaseOrderInput

| Field | Type | Description |
|---|---|---|
| `supplierId` | `string` |  |
| `items` | `Array<CreatePurchaseOrderItemInput>` | Types: [`CreatePurchaseOrderItemInput`](#createpurchaseorderiteminput) |
| `notes?` | `string` |  |

### CreatePurchaseOrderItemInput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name` | `string` |  |
| `quantity` | `number` |  |
| `unitCost` | `number` |  |

### CreateQualityHoldInput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `lotNumber?` | `string` |  |
| `quantityHeld` | `number` |  |
| `reason` | `string` |  |
| `holdType` | `QualityHoldTypeInput` | Types: [`QualityHoldTypeInput`](#qualityholdtypeinput) |
| `placedBy?` | `string` |  |
| `locationId?` | `number` |  |

### CreateReceiptInput

| Field | Type | Description |
|---|---|---|
| `receiptType` | `ReceiptTypeInput` | Types: [`ReceiptTypeInput`](#receipttypeinput) |
| `warehouseId` | `number` |  |
| `purchaseOrderId?` | `string` |  |
| `carrier?` | `string` |  |
| `trackingNumber?` | `string` |  |

### CreateRefundExactInput

Exact-money refund input for agent and financial integrations.

| Field | Type | Description |
|---|---|---|
| `paymentId` | `string` |  |
| `amount` | `string` |  |
| `reason?` | `string` |  |
| `idempotencyKey?` | `string` |  |

### CreateRefundInput

| Field | Type | Description |
|---|---|---|
| `paymentId` | `string` |  |
| `amount?` | `number` | Float amount. Optional: send `amount_exact` instead for exact money. |
| `amountExact?` | `string` | Exact base-10 amount. Takes precedence over `amount` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `reason?` | `string` |  |
| `idempotencyKey?` | `string` |  |

### CreateReturnInput

| Field | Type | Description |
|---|---|---|
| `orderId` | `string` |  |
| `reason` | `ReturnReason` | An unrecognised reason is refused with `VALIDATION`; send `other` explicitly. Types: [`ReturnReason`](#returnreason) |
| `reasonDetails?` | `string` |  |
| `idempotencyKey?` | `string` |  |
| `items` | `Array<CreateReturnItemInput>` | Types: [`CreateReturnItemInput`](#createreturniteminput) |

### CreateReturnItemInput

| Field | Type | Description |
|---|---|---|
| `orderItemId` | `string` |  |
| `quantity` | `number` |  |

### CreateRevenueContractInput

| Field | Type | Description |
|---|---|---|
| `contractNumber?` | `string` | Optional contract number; auto-generated when omitted (RC-...) |
| `customerId` | `string` |  |
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `transactionPrice` | `string` | Exact decimal string |
| `currency?` | `string` | Currency code, e.g. "USD" |
| `effectiveDate` | `string` | ISO date (YYYY-MM-DD) |
| `obligations` | `Array<CreatePerformanceObligationInput>` | Types: [`CreatePerformanceObligationInput`](#createperformanceobligationinput) |

### CreateReviewInput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `customerId` | `string` |  |
| `rating` | `number` | Star rating 1–5 |
| `title?` | `string` |  |
| `body?` | `string` |  |
| `verifiedPurchase?` | `boolean` |  |

### CreateRewardInput

| Field | Type | Description |
|---|---|---|
| `programId` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `pointsCost` | `number` |  |
| `rewardType` | `LoyaltyRewardType` | Reward type, e.g. "discount", "free_product", "free_shipping" Types: [`LoyaltyRewardType`](#loyaltyrewardtype) |
| `value?` | `string` | Monetary value as an exact decimal string (optional) |

### CreateSearchConfigInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `description?` | `string` |  |
| `searchableFields?` | `Array<SearchFieldInput>` | Types: [`SearchFieldInput`](#searchfieldinput) |
| `facets?` | `Array<FacetConfigInput>` | Types: [`FacetConfigInput`](#facetconfiginput) |
| `synonyms?` | `Array<SynonymGroupInput>` | Types: [`SynonymGroupInput`](#synonymgroupinput) |
| `boostRules?` | `Array<BoostRuleInput>` | Types: [`BoostRuleInput`](#boostruleinput) |

### CreateSegmentInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `description?` | `string` |  |
| `segmentType?` | `SegmentType` | "static" (default) or "dynamic" Types: [`SegmentType`](#segmenttype) |
| `rules?` | `Array<SegmentRuleInput>` | Types: [`SegmentRuleInput`](#segmentruleinput) |

### CreateSerialInput

| Field | Type | Description |
|---|---|---|
| `serial?` | `string` |  |
| `sku` | `string` |  |
| `lotNumber?` | `string` |  |
| `manufacturedAt?` | `string` |  |

### CreateShipmentInput

| Field | Type | Description |
|---|---|---|
| `orderId` | `string` |  |
| `recipientName` | `string` |  |
| `shippingAddress` | `string` |  |
| `carrier?` | `ShippingCarrierInput` | An unrecognised carrier is refused with `VALIDATION`; send `other` explicitly. Types: [`ShippingCarrierInput`](#shippingcarrierinput) |
| `shippingMethod?` | `ShipmentMethodInput` | An unrecognised method is refused with `VALIDATION`. Types: [`ShipmentMethodInput`](#shipmentmethodinput) |
| `trackingNumber?` | `string` |  |
| `recipientEmail?` | `string` |  |
| `recipientPhone?` | `string` |  |

### CreateShippingZoneInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `countries?` | `Array<string>` |  |
| `regions?` | `Array<string>` |  |
| `postalCodes?` | `Array<string>` |  |
| `priority?` | `number` |  |

### CreateStoreCreditInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` | Customer UUID that owns the credit |
| `amount` | `string` | Amount to issue as an exact decimal string, e.g. "25.00" |
| `currency` | `string` | Currency code, e.g. "USD" |
| `reason?` | `StoreCreditReason` | Reason: return, loyalty, compensation, promotion, manual, gift_card (defaults to "return") Types: [`StoreCreditReason`](#storecreditreason) |
| `referenceId?` | `string` |  |
| `note?` | `string` |  |
| `expiresAt?` | `string` | RFC 3339 expiry timestamp (None = never expires) |

### CreateSubscriptionInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `planId` | `string` |  |
| `paymentMethodId?` | `string` |  |
| `skipTrial?` | `boolean` |  |
| `price?` | `number` |  |
| `couponCode?` | `string` |  |
| `startDate?` | `string` |  |

### CreateSubscriptionPlanInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `description?` | `string` |  |
| `code?` | `string` |  |
| `billingInterval` | `SubscriptionBillingIntervalInput` | Types: [`SubscriptionBillingIntervalInput`](#subscriptionbillingintervalinput) |
| `customIntervalDays?` | `number` |  |
| `price` | `number` |  |
| `setupFee?` | `number` |  |
| `currency?` | `string` |  |
| `trialDays?` | `number` |  |
| `trialRequiresPaymentMethod?` | `boolean` |  |
| `minCycles?` | `number` |  |
| `maxCycles?` | `number` |  |
| `discountPercent?` | `number` |  |
| `discountAmount?` | `number` |  |

### CreateSupplierInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `supplierCode?` | `string` |  |
| `email?` | `string` |  |
| `phone?` | `string` |  |

### CreateSupplierSkuInput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `supplierId` | `string` |  |
| `sku` | `string` |  |
| `unitCost?` | `string` | Exact decimal string |
| `currency?` | `string` | Currency code, e.g. "USD" |
| `minOrderQty?` | `string` | Exact decimal string |
| `leadTimeDays?` | `number` |  |

### CreateTaxRateInput

| Field | Type | Description |
|---|---|---|
| `jurisdictionId` | `string` |  |
| `taxType?` | `TaxType` | Types: [`TaxType`](#taxtype) |
| `productCategory?` | `ProductTaxCategory` | Types: [`ProductTaxCategory`](#producttaxcategory) |
| `rate` | `number` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `isCompound?` | `boolean` |  |
| `priority?` | `number` |  |
| `thresholdMin?` | `number` |  |
| `thresholdMax?` | `number` |  |
| `fixedAmount?` | `number` |  |
| `effectiveFrom` | `string` |  |
| `effectiveTo?` | `string` |  |

### CreateTransferOrderInput

| Field | Type | Description |
|---|---|---|
| `sourceWarehouseId` | `string` |  |
| `destinationWarehouseId` | `string` |  |
| `items` | `Array<CreateTransferOrderItemInput>` | Types: [`CreateTransferOrderItemInput`](#createtransferorderiteminput) |
| `expectedAt?` | `string` | RFC 3339 timestamp |
| `notes?` | `string` |  |

### CreateTransferOrderItemInput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `quantity` | `string` | Exact decimal string |

### CreateUnitClassInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `description?` | `string` |  |

### CreateUnitConversionRuleInput

| Field | Type | Description |
|---|---|---|
| `ruleType` | `ConversionRuleType` | SYSTEM or SKU Types: [`ConversionRuleType`](#conversionruletype) |
| `productId?` | `string` |  |
| `fromUomId` | `string` |  |
| `toUomId` | `string` |  |
| `factor` | `string` | Exact decimal string |

### CreateUnitOfMeasureInput

| Field | Type | Description |
|---|---|---|
| `unitClassId` | `string` |  |
| `name` | `string` |  |
| `abbreviation` | `string` |  |
| `factor` | `string` | Exact decimal string relative to the class base unit |

### CreateVendorCreditInput

| Field | Type | Description |
|---|---|---|
| `supplierId` | `string` |  |
| `vendorReturnId?` | `string` |  |
| `amount` | `string` | Exact decimal string |
| `currency?` | `string` | Currency code, e.g. "USD" |
| `memo?` | `string` |  |

### CreateVendorReturnInput

| Field | Type | Description |
|---|---|---|
| `supplierId` | `string` |  |
| `purchaseOrderId?` | `string` |  |
| `currency?` | `string` |  |
| `items` | `Array<CreateVendorReturnItemInput>` | Types: [`CreateVendorReturnItemInput`](#createvendorreturniteminput) |
| `notes?` | `string` |  |

### CreateVendorReturnItemInput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `quantity` | `string` | Exact decimal string |
| `unitCost` | `string` | Exact decimal string |
| `reason?` | `VendorReturnReason` | Snake-case reason: `defective`, `overage`, `wrong_item`, `other` Types: [`VendorReturnReason`](#vendorreturnreason) |

### CreateWarehouseInput

| Field | Type | Description |
|---|---|---|
| `code` | `string` |  |
| `name` | `string` |  |
| `warehouseType?` | `WarehouseTypeInput` | Types: [`WarehouseTypeInput`](#warehousetypeinput) |
| `timezone?` | `string` |  |

### CreateWarrantyClaimInput

| Field | Type | Description |
|---|---|---|
| `warrantyId` | `string` |  |
| `issueDescription` | `string` |  |
| `contactEmail?` | `string` |  |
| `contactPhone?` | `string` |  |

### CreateWarrantyInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `productId?` | `string` |  |
| `orderId?` | `string` |  |
| `warrantyType?` | `WarrantyTypeInput` | Anything other than a recognised tier uses the engine default (`standard`). Types: [`WarrantyTypeInput`](#warrantytypeinput) |
| `durationMonths?` | `number` |  |
| `serialNumber?` | `string` |  |

### CreateWaveInput

| Field | Type | Description |
|---|---|---|
| `warehouseId` | `number` |  |
| `orderIds` | `Array<string>` |  |
| `priority?` | `number` |  |
| `notes?` | `string` |  |

### CreateWebhookInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` | Display name |
| `url` | `string` | Target URL for POST requests |
| `secret?` | `string` | Optional secret for HMAC signature |
| `eventTypes?` | `Array<string>` | Event types to receive (empty or omitted = all events) |

### CreateWishlistInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `name` | `string` |  |
| `isPublic?` | `boolean` |  |

### CreateWorkOrderInput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `bomId?` | `string` |  |
| `quantityToBuild` | `number` |  |
| `priority?` | `WorkOrderPriority` | Anything other than a recognised priority uses the engine default (`normal`). Types: [`WorkOrderPriority`](#workorderpriority) |
| `notes?` | `string` |  |

### CreateZoneShippingMethodInput

| Field | Type | Description |
|---|---|---|
| `zoneId` | `string` |  |
| `name` | `string` |  |
| `carrier?` | `string` |  |
| `methodType` | `ShippingMethodType` | flat, weight_based, price_based, calculated, free Types: [`ShippingMethodType`](#shippingmethodtype) |
| `baseRate` | `string` | Exact decimal string |
| `currency` | `string` | ISO 4217 currency code |
| `minDeliveryDays?` | `number` |  |
| `maxDeliveryDays?` | `number` |  |
| `conditions?` | `Array<ShippingConditionInput>` | Types: [`ShippingConditionInput`](#shippingconditioninput) |

### CreditAccountFilterInput

Optional filters for `Credit.listCreditAccounts`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `status?` | `CreditAccountStatusInput` | The rendered form (`OnHold`) or the engine's snake_case (`on_hold`). Types: [`CreditAccountStatusInput`](#creditaccountstatusinput) |
| `overLimit?` | `boolean` | Only accounts whose balance exceeds their limit. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### CreditAccountOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `customerId` | `string` |  |
| `creditLimit` | `number` | **Deprecated.** Use the `creditLimitExact` twin; float money will be removed in 2.0. |
| `creditLimitExact` | `string` | Exact base-10 credit limit, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `creditUsed` | `number` | **Deprecated.** Use the `creditUsedExact` twin; float money will be removed in 2.0. |
| `creditUsedExact` | `string` | Exact base-10 credit used, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `creditAvailable` | `number` | **Deprecated.** Use the `creditAvailableExact` twin; float money will be removed in 2.0. |
| `creditAvailableExact` | `string` | Exact base-10 credit available, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `status` | `CreditAccountStatus` | Types: [`CreditAccountStatus`](#creditaccountstatus) |
| `paymentTerms?` | `string` |  |

### CreditAccountStatus

Customer credit account status as rendered on `CreditAccountOutput.status` (Rust `Debug` form).

```ts
type CreditAccountStatus = 'Active' | 'Suspended' | 'OnHold' | 'Closed' | 'PendingReview'
```

One of: `'Active'`, `'Suspended'`, `'OnHold'`, `'Closed'`, `'PendingReview'`.

### CreditAccountStatusInput

Credit account status accepted by `CreditAccountFilterInput.status`: the rendered form or the engine's snake_case (strict).

```ts
type CreditAccountStatusInput = CreditAccountStatus | 'active' | 'suspended' | 'on_hold' | 'onhold' | 'closed' | 'pending_review' | 'pendingreview'
```

Types: [`CreditAccountStatus`](#creditaccountstatus)

### CreditCheckOutput

| Field | Type | Description |
|---|---|---|
| `approved` | `boolean` |  |
| `reason?` | `string` |  |
| `availableCredit` | `number` | **Deprecated.** Use the `availableCreditExact` twin; float money will be removed in 2.0. |
| `availableCreditExact` | `string` | Exact base-10 available credit, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `requiresApproval` | `boolean` |  |

### CreditMemoFilterInput

Optional filters for `AccountsReceivable.listCreditMemos`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `status?` | `CreditMemoStatusInput` | The rendered form (`PartiallyApplied`) or the engine's snake_case (`partially_applied`). Types: [`CreditMemoStatusInput`](#creditmemostatusinput) |
| `reason?` | `CreditMemoReasonFilter` | The rendered form (`ReturnedGoods`) or the engine's snake_case (`returned_goods`). Types: [`CreditMemoReasonFilter`](#creditmemoreasonfilter) |
| `hasUnapplied?` | `boolean` |  |
| `fromDate?` | `string` | RFC 3339 timestamp. |
| `toDate?` | `string` | RFC 3339 timestamp. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### CreditMemoOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `creditMemoNumber` | `string` |  |
| `customerId` | `string` |  |
| `amount` | `number` | **Deprecated.** Use the `amountExact` twin; float money will be removed in 2.0. |
| `amountExact` | `string` | Exact base-10 amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `status` | `CreditMemoStatus` | Types: [`CreditMemoStatus`](#creditmemostatus) |
| `reason` | `CreditMemoReason` | Types: [`CreditMemoReason`](#creditmemoreason) |
| `createdAt` | `string` |  |

### CreditMemoReason

Credit memo reason as rendered on `CreditMemoOutput.reason` (Rust `Debug` form).

```ts
type CreditMemoReason = 'ReturnedGoods' | 'PricingError' | 'Overpayment' | 'Damaged' | 'ServiceCredit' | 'GoodwillAdjustment' | 'Other'
```

One of: `'ReturnedGoods'`, `'PricingError'`, `'Overpayment'`, `'Damaged'`, `'ServiceCredit'`, `'GoodwillAdjustment'`, `'Other'`.

### CreditMemoReasonFilter

Credit memo reason accepted by `CreditMemoFilterInput.reason`: the rendered form or the engine's snake_case (strict).

```ts
type CreditMemoReasonFilter = CreditMemoReason | 'returned_goods' | 'pricing_error' | 'overpayment' | 'damaged' | 'service_credit' | 'goodwill_adjustment' | 'other'
```

Types: [`CreditMemoReason`](#creditmemoreason)

### CreditMemoReasonInput

Credit memo reason accepted by `CreateCreditMemoInput.reason` (case-insensitive).

```ts
type CreditMemoReasonInput = 'returned_goods' | 'returnedgoods' | 'return' | 'pricing_error' | 'pricingerror' | 'billing_error' | 'overpayment' | 'damaged' | 'service_credit' | 'servicecredit' | 'goodwill' | 'goodwill_adjustment' | 'other'
```

One of: `'returned_goods'`, `'returnedgoods'`, `'return'`, `'pricing_error'`, `'pricingerror'`, `'billing_error'`, `'overpayment'`, `'damaged'`, `'service_credit'`, `'servicecredit'`, `'goodwill'`, `'goodwill_adjustment'`, `'other'`.

### CreditMemoStatus

Credit memo status as rendered on `CreditMemoOutput.status` (Rust `Debug` form).

```ts
type CreditMemoStatus = 'Open' | 'PartiallyApplied' | 'FullyApplied' | 'Voided'
```

One of: `'Open'`, `'PartiallyApplied'`, `'FullyApplied'`, `'Voided'`.

### CreditMemoStatusInput

Credit memo status accepted by `CreditMemoFilterInput.status`: the rendered form or the engine's snake_case (strict).

```ts
type CreditMemoStatusInput = CreditMemoStatus | 'open' | 'partially_applied' | 'fully_applied' | 'voided'
```

Types: [`CreditMemoStatus`](#creditmemostatus)

### CustomFieldDefinitionInput

| Field | Type | Description |
|---|---|---|
| `key` | `string` |  |
| `fieldType` | `CustomFieldTypeInput` | Types: [`CustomFieldTypeInput`](#customfieldtypeinput) |
| `required?` | `boolean` |  |
| `list?` | `boolean` |  |
| `description?` | `string` |  |

### CustomFieldDefinitionOutput

| Field | Type | Description |
|---|---|---|
| `key` | `string` |  |
| `fieldType` | `CustomFieldType` | Types: [`CustomFieldType`](#customfieldtype) |
| `required` | `boolean` |  |
| `list` | `boolean` |  |
| `description?` | `string` |  |

### CustomFieldType

Custom object field type as rendered on `CustomFieldDefinitionOutput.fieldType`.

```ts
type CustomFieldType = 'string' | 'integer' | 'decimal' | 'boolean' | 'date_time' | 'uuid' | 'json'
```

One of: `'string'`, `'integer'`, `'decimal'`, `'boolean'`, `'date_time'`, `'uuid'`, `'json'`.

### CustomFieldTypeInput

Every spelling `CustomFieldDefinitionInput.fieldType` accepts (the short aliases normalise to the canonical form).

```ts
type CustomFieldTypeInput = CustomFieldType | 'int' | 'number' | 'bool' | 'datetime'
```

Types: [`CustomFieldType`](#customfieldtype)

### CustomObjectFilterInput

| Field | Type | Description |
|---|---|---|
| `typeHandle?` | `string` |  |
| `ownerType?` | `string` |  |
| `ownerId?` | `string` |  |
| `handle?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### CustomObjectOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `typeId` | `string` |  |
| `typeHandle` | `string` |  |
| `handle?` | `string` |  |
| `ownerType?` | `string` |  |
| `ownerId?` | `string` |  |
| `valuesJson` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |
| `version` | `number` |  |

### CustomObjectTypeFilterInput

| Field | Type | Description |
|---|---|---|
| `search?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### CustomObjectTypeOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `handle` | `string` |  |
| `displayName` | `string` |  |
| `description` | `string` |  |
| `fields` | `Array<CustomFieldDefinitionOutput>` | Types: [`CustomFieldDefinitionOutput`](#customfielddefinitionoutput) |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |
| `version` | `number` |  |

### CustomerAddressOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `customerId` | `string` |  |
| `addressType` | `AddressType` | Types: [`AddressType`](#addresstype) |
| `firstName` | `string` |  |
| `lastName` | `string` |  |
| `company?` | `string` |  |
| `line1` | `string` |  |
| `line2?` | `string` |  |
| `city` | `string` |  |
| `state?` | `string` |  |
| `postalCode` | `string` |  |
| `country` | `string` |  |
| `phone?` | `string` |  |
| `isDefault` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### CustomerFilterInput

Filter for `customers.list()`

| Field | Type | Description |
|---|---|---|
| `email?` | `string` | Exact e-mail match |
| `status?` | `CustomerStatus` | Account status Types: [`CustomerStatus`](#customerstatus) |
| `tag?` | `string` | Customers carrying this tag |
| `acceptsMarketing?` | `boolean` |  |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |
| `afterCursor?` | `Array<string>` | Keyset cursor: `[createdAt, id]` |

### CustomerMetadata

Free-form JSON metadata attached to a customer. Any JSON object; the engine stores it verbatim.

```ts
type CustomerMetadata = Record<string, unknown>
```

### CustomerMetricsOutput

| Field | Type | Description |
|---|---|---|
| `totalCustomers` | `number` |  |
| `newCustomers` | `number` |  |
| `returningCustomers` | `number` |  |
| `averageLifetimeValue` | `number` | **Deprecated.** Use the `averageLifetimeValueExact` twin; float money will be removed in 2.0. |
| `averageLifetimeValueExact` | `string` | Exact base-10 average lifetime value, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `averageOrdersPerCustomer` | `number` |  |

### CustomerOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `email` | `string` |  |
| `firstName` | `string` |  |
| `lastName` | `string` |  |
| `phone?` | `string` |  |
| `status` | `CustomerStatus` | Types: [`CustomerStatus`](#customerstatus) |
| `acceptsMarketing` | `boolean` |  |
| `emailVerified` | `boolean` |  |
| `tags` | `Array<string>` |  |
| `metadata?` | `CustomerMetadata` | Types: [`CustomerMetadata`](#customermetadata) |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### CustomerSearchResultOutput

| Field | Type | Description |
|---|---|---|
| `customer` | `CustomerOutput` | Types: [`CustomerOutput`](#customeroutput) |
| `distance` | `number` |  |
| `score` | `number` |  |

### CustomerStatus

Customer account status (`CustomerOutput.status`, `UpdateCustomerInput.status`, `CustomerFilterInput.status`).

```ts
type CustomerStatus = 'active' | 'inactive' | 'suspended' | 'deleted'
```

One of: `'active'`, `'inactive'`, `'suspended'`, `'deleted'`.

### CycleCountFilterInput

| Field | Type | Description |
|---|---|---|
| `warehouseId?` | `number` |  |
| `locationId?` | `number` |  |
| `status?` | `CycleCountStatus` | draft, in_progress, completed, cancelled Types: [`CycleCountStatus`](#cyclecountstatus) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### CycleCountLineOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `cycleCountId` | `string` |  |
| `sku` | `string` |  |
| `lotId?` | `string` |  |
| `expectedQuantity` | `string` | Exact decimal string |
| `countedQuantity?` | `string` | Exact decimal string |
| `variance?` | `string` | Exact decimal string: counted_quantity - expected_quantity |

### CycleCountOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `warehouseId` | `number` |  |
| `locationId?` | `number` |  |
| `status` | `CycleCountStatus` | draft, in_progress, completed, cancelled Types: [`CycleCountStatus`](#cyclecountstatus) |
| `scheduledDate?` | `string` |  |
| `countedBy?` | `string` |  |
| `lines` | `Array<CycleCountLineOutput>` | Types: [`CycleCountLineOutput`](#cyclecountlineoutput) |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |
| `completedAt?` | `string` |  |

### CycleCountStatus

Cycle count lifecycle status (`CycleCountStatus`).

```ts
type CycleCountStatus = 'draft' | 'in_progress' | 'completed' | 'cancelled'
```

One of: `'draft'`, `'in_progress'`, `'completed'`, `'cancelled'`.

### DemandForecastOutput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name` | `string` |  |
| `averageDailyDemand` | `number` |  |
| `forecastedDemand` | `number` |  |
| `confidence` | `number` |  |
| `currentStock` | `number` |  |
| `daysUntilStockout?` | `number` |  |
| `recommendedReorderQty?` | `number` |  |
| `trend` | `DemandTrend` | Types: [`DemandTrend`](#demandtrend) |

### DemandTrend

Demand direction on `DemandForecastOutput.trend` (rendered in the engine's `Debug` casing).

```ts
type DemandTrend = 'Rising' | 'Stable' | 'Falling'
```

One of: `'Rising'`, `'Stable'`, `'Falling'`.

### DepreciationEntryOutput

| Field | Type | Description |
|---|---|---|
| `period` | `number` |  |
| `amount` | `string` | Exact decimal string |
| `accumulated` | `string` | Exact decimal string |
| `bookValue` | `string` | Exact decimal string |
| `status` | `DepreciationEntryStatus` | scheduled or posted Types: [`DepreciationEntryStatus`](#depreciationentrystatus) |

### DepreciationEntryStatus

Depreciation schedule entry status (`DepreciationEntryStatus`).

```ts
type DepreciationEntryStatus = 'scheduled' | 'posted'
```

One of: `'scheduled'`, `'posted'`.

### DepreciationMethod

Depreciation method accepted on input (exact, lowercase). `declining_balance` also needs `decliningBalanceRate`.

```ts
type DepreciationMethod = 'straight_line' | 'declining_balance' | 'units_of_production'
```

One of: `'straight_line'`, `'declining_balance'`, `'units_of_production'`.

### DepreciationMethodOutput

Depreciation method as rendered on a record; `unknown` only for a method this binding predates.

```ts
type DepreciationMethodOutput = DepreciationMethod | 'unknown'
```

Types: [`DepreciationMethod`](#depreciationmethod)

### DepreciationScheduleOutput

| Field | Type | Description |
|---|---|---|
| `assetId` | `string` |  |
| `method` | `DepreciationMethodOutput` | straight_line, declining_balance, units_of_production Types: [`DepreciationMethodOutput`](#depreciationmethodoutput) |
| `decliningBalanceRate?` | `string` | Set when method is declining_balance |
| `entries` | `Array<DepreciationEntryOutput>` | Types: [`DepreciationEntryOutput`](#depreciationentryoutput) |
| `totalDepreciation` | `string` | Exact decimal string |

### DomainCountOutput

Per-domain record count.

| Field | Type | Description |
|---|---|---|
| `domain` | `string` |  |
| `count` | `number` |  |

### EconomicBudget

Operator-provisioned monetary authority for one principal
(`EconomicBudget`). Immutable once provisioned: re-provisioning the same
definition is idempotent, changing it under the same id is rejected.

| Field | Type | Description |
|---|---|---|
| `budget_id` | `string` |  |
| `principal_id` | `string` |  |
| `tenant_id?` | `string \| null` |  |
| `store_id?` | `string \| null` |  |
| `limit` | `MoneyWire` | Total exact amount available over the budget lifetime. Types: [`MoneyWire`](#moneywire) |
| `valid_from` | `string` |  |
| `expires_at` | `string` |  |

### EconomicBudgetStatus

Exact balances of a provisioned budget (`EconomicBudgetStatus`).

| Field | Type | Description |
|---|---|---|
| `budget` | `EconomicBudget & { tenant_id: string \| null; store_id: string \| null }` | The definition as stored; optional scope fields come back as `null`. Types: [`EconomicBudget`](#economicbudget) |
| `committed` | `MoneyWire` | Reserved by successfully committed commands. Types: [`MoneyWire`](#moneywire) |
| `available` | `MoneyWire` | Still available for new commitments. Types: [`MoneyWire`](#moneywire) |

### EconomicCommitment

Resources an economic command proposes to commit (`EconomicCommitment`).

| Field | Type | Description |
|---|---|---|
| `budget_id?` | `string \| null` | Budget provisioned via `provisionEconomicBudget`. |
| `amount?` | `MoneyWire \| null` | Exact fiat money placed at risk. Mutually exclusive with `asset_amount`. Types: [`MoneyWire`](#moneywire) |
| `asset_amount?` | `AssetAmountWire \| null` | Types: [`AssetAmountWire`](#assetamountwire) |
| `counterparty_id?` | `string \| null` |  |
| `quantity?` | `string \| null` | Exact unit commitment as a decimal string; units may be fractional. |
| `evidence?` | `string[]` | Evidence identifiers (quotes, tickets, contracts) supporting the action. |

### EconomicMandate

A principal-issued objective an agent acts under (`EconomicMandate`).

| Field | Type | Description |
|---|---|---|
| `mandate_id` | `string` |  |
| `subject_id` | `string` |  |
| `issued_by` | `string` |  |
| `objective` | `string` |  |
| `allowed_commands?` | `string[]` | Commands this mandate permits. Empty is never a wildcard. |
| `tenant_id?` | `string \| null` |  |
| `store_id?` | `string \| null` |  |
| `issued_at` | `string` |  |
| `expires_at` | `string` |  |

### EdiCountOutput

| Field | Type | Description |
|---|---|---|
| `key` | `string` | The group key (status or document type) |
| `count` | `number` | Number of documents in the group |

### EdiDirection

EDI document direction relative to this system (`EdiDirection`).

```ts
type EdiDirection = 'inbound' | 'outbound'
```

One of: `'inbound'`, `'outbound'`.

### EdiDocumentFilterInput

| Field | Type | Description |
|---|---|---|
| `documentType?` | `string` | Filter by document type (e.g. `850`) |
| `direction?` | `EdiDirection` | Filter by direction: `inbound` or `outbound` Types: [`EdiDirection`](#edidirection) |
| `status?` | `EdiStatus` | Filter by status: `pending`, `sent`, `acknowledged`, `processed`, `error` Types: [`EdiStatus`](#edistatus) |
| `partner?` | `string` | Filter by trading partner |
| `limit?` | `number` | Maximum results |
| `offset?` | `number` | Offset for pagination |

### EdiDocumentOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `documentType` | `string` | EDI document type (e.g. `850`, `855`, `856`, `810`) |
| `direction` | `EdiDirection` | One of `inbound`, `outbound` Types: [`EdiDirection`](#edidirection) |
| `status` | `EdiStatus` | One of `pending`, `sent`, `acknowledged`, `processed`, `error` Types: [`EdiStatus`](#edistatus) |
| `partner?` | `string` | Trading partner name / id |
| `reference?` | `string` | Related business reference (PO number, order number, etc.) |
| `payload?` | `string` | Raw EDI payload |
| `errorMessage?` | `string` | Error detail when `status = error` |
| `createdAt` | `string` | RFC 3339 timestamp |
| `updatedAt` | `string` | RFC 3339 timestamp |

### EdiStatus

EDI document processing status (`EdiStatus`).

```ts
type EdiStatus = 'pending' | 'sent' | 'acknowledged' | 'processed' | 'error'
```

One of: `'pending'`, `'sent'`, `'acknowledged'`, `'processed'`, `'error'`.

### EdiSummaryOutput

| Field | Type | Description |
|---|---|---|
| `total` | `number` | Total document count |
| `byStatus` | `Array<EdiCountOutput>` | Counts grouped by status Types: [`EdiCountOutput`](#edicountoutput) |
| `byType` | `Array<EdiCountOutput>` | Counts grouped by document type Types: [`EdiCountOutput`](#edicountoutput) |

### EmbeddingStatsOutput

| Field | Type | Description |
|---|---|---|
| `productCount` | `number` |  |
| `customerCount` | `number` |  |
| `orderCount` | `number` |  |
| `inventoryCount` | `number` |  |
| `model` | `string` |  |
| `dimensions` | `number` |  |

### EnqueuePrintJobInput

| Field | Type | Description |
|---|---|---|
| `printerName?` | `string` |  |
| `payloadKind?` | `PrintPayloadKind` | zpl or pdf Types: [`PrintPayloadKind`](#printpayloadkind) |
| `payload` | `string` |  |

### EnrollCustomerInput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `programId` | `string` |  |

### EuVatInfoOutput

| Field | Type | Description |
|---|---|---|
| `countryCode` | `string` |  |
| `countryName` | `string` |  |
| `standardRate` | `number` |  |
| `reducedRate?` | `number` |  |
| `superReducedRate?` | `number` |  |
| `parkingRate?` | `number` |  |

### ExchangeRateFilterInput

| Field | Type | Description |
|---|---|---|
| `baseCurrency?` | `string` | Filter by base currency |
| `quoteCurrency?` | `string` | Filter by quote currency |

### ExchangeRateOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `baseCurrency` | `string` |  |
| `quoteCurrency` | `string` |  |
| `rate` | `number` |  |
| `source` | `string` |  |
| `rateAt` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ExemptionDetailsOutput

| Field | Type | Description |
|---|---|---|
| `exemptionId` | `string` |  |
| `exemptionType` | `TaxExemptionType` | Types: [`TaxExemptionType`](#taxexemptiontype) |
| `certificateNumber?` | `string` |  |
| `amountExempt` | `number` | **Deprecated.** Use the `amountExemptExact` twin; float money will be removed in 2.0. |
| `amountExemptExact` | `string` | Exact base-10 amount exempt, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `taxSaved` | `number` | **Deprecated.** Use the `taxSavedExact` twin; float money will be removed in 2.0. |
| `taxSavedExact` | `string` | Exact base-10 tax saved, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### ExportOptionsInput

Options controlling a structured export.

| Field | Type | Description |
|---|---|---|
| `domains?` | `Array<string>` |  |
| `pageSize?` | `number` |  |
| `pretty?` | `boolean` |  |

### ExportReportOutput

Result of a structured export.

| Field | Type | Description |
|---|---|---|
| `counts` | `Array<DomainCountOutput>` | Types: [`DomainCountOutput`](#domaincountoutput) |
| `total` | `number` |  |

### FacetConfigInput

| Field | Type | Description |
|---|---|---|
| `fieldName` | `string` |  |
| `facetType?` | `FacetType` | Snake-case facet type: `value`, `range`, `hierarchical` Types: [`FacetType`](#facettype) |
| `displayName` | `string` |  |
| `sortOrder?` | `number` |  |
| `maxValues?` | `number` |  |

### FacetConfigOutput

| Field | Type | Description |
|---|---|---|
| `fieldName` | `string` |  |
| `facetType` | `FacetType` | Snake-case facet type Types: [`FacetType`](#facettype) |
| `displayName` | `string` |  |
| `sortOrder` | `number` |  |
| `maxValues?` | `number` |  |

### FacetType

Facet type for search refinement (`FacetType`).

```ts
type FacetType = 'value' | 'range' | 'hierarchical'
```

One of: `'value'`, `'range'`, `'hierarchical'`.

### FeedbackSummaryOutput

| Field | Type | Description |
|---|---|---|
| `count` | `string` | Count as a decimal string |
| `summaryValue` | `string` | Aggregate value as a decimal string |
| `summaryValueDecimals` | `number` |  |

### FieldTransform

Value transform applied by an integration field mapping (`FieldTransform`).

```ts
type FieldTransform = 'none' | 'uppercase' | 'lowercase' | 'trim'
```

One of: `'none'`, `'uppercase'`, `'lowercase'`, `'trim'`.

### FixedAssetCategory

Fixed-asset category (`FixedAssetCategory`).

```ts
type FixedAssetCategory = 'land' | 'building' | 'machinery' | 'equipment' | 'vehicle' | 'furniture_and_fixtures' | 'computer_hardware' | 'software' | 'leasehold_improvement' | 'other'
```

One of: `'land'`, `'building'`, `'machinery'`, `'equipment'`, `'vehicle'`, `'furniture_and_fixtures'`, `'computer_hardware'`, `'software'`, `'leasehold_improvement'`, `'other'`.

### FixedAssetFilterInput

| Field | Type | Description |
|---|---|---|
| `category?` | `FixedAssetCategory` | Types: [`FixedAssetCategory`](#fixedassetcategory) |
| `status?` | `FixedAssetStatus` | draft, in_service, fully_depreciated, disposed, written_off Types: [`FixedAssetStatus`](#fixedassetstatus) |
| `locationId?` | `string` |  |
| `acquiredFrom?` | `string` | ISO date (YYYY-MM-DD) |
| `acquiredTo?` | `string` | ISO date (YYYY-MM-DD) |
| `search?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### FixedAssetOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `assetNumber` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `category` | `FixedAssetCategory` | Types: [`FixedAssetCategory`](#fixedassetcategory) |
| `acquisitionDate` | `string` | ISO date (YYYY-MM-DD) |
| `acquisitionCost` | `string` | Exact decimal string |
| `salvageValue` | `string` | Exact decimal string |
| `usefulLifeMonths` | `number` |  |
| `depreciationMethod` | `DepreciationMethodOutput` | straight_line, declining_balance, units_of_production Types: [`DepreciationMethodOutput`](#depreciationmethodoutput) |
| `decliningBalanceRate?` | `string` | Set when depreciation_method is declining_balance |
| `status` | `FixedAssetStatus` | draft, in_service, fully_depreciated, disposed, written_off Types: [`FixedAssetStatus`](#fixedassetstatus) |
| `inServiceDate?` | `string` | ISO date (YYYY-MM-DD) |
| `locationId?` | `string` |  |
| `assetAccountId?` | `string` |  |
| `accumulatedDepreciationAccountId?` | `string` |  |
| `depreciationExpenseAccountId?` | `string` |  |
| `accumulatedDepreciation` | `string` | Exact decimal string |
| `bookValue` | `string` | Exact decimal string: acquisition_cost - accumulated_depreciation |
| `currency` | `string` |  |
| `disposal?` | `AssetDisposalOutput` | Types: [`AssetDisposalOutput`](#assetdisposaloutput) |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### FixedAssetStatus

Fixed-asset lifecycle status (`FixedAssetStatus`).

```ts
type FixedAssetStatus = 'draft' | 'in_service' | 'fully_depreciated' | 'disposed' | 'written_off'
```

One of: `'draft'`, `'in_service'`, `'fully_depreciated'`, `'disposed'`, `'written_off'`.

### FraudAssessmentFilterInput

| Field | Type | Description |
|---|---|---|
| `decision?` | `FraudDecision` | Snake-case decision: `accept`, `review`, `reject` Types: [`FraudDecision`](#frauddecision) |
| `minRiskScore?` | `number` |  |
| `unreviewedOnly?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### FraudAssessmentOutput

| Field | Type | Description |
|---|---|---|
| `orderId` | `string` |  |
| `riskScore` | `number` |  |
| `signals` | `Array<FraudSignalOutput>` | Types: [`FraudSignalOutput`](#fraudsignaloutput) |
| `decision` | `FraudDecision` | Snake-case decision Types: [`FraudDecision`](#frauddecision) |
| `reviewedBy?` | `string` |  |
| `reviewNotes?` | `string` |  |
| `needsReview` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### FraudDecision

Fraud assessment decision, also a rule's action (`FraudDecision`).

```ts
type FraudDecision = 'accept' | 'review' | 'reject'
```

One of: `'accept'`, `'review'`, `'reject'`.

### FraudRuleFilterInput

| Field | Type | Description |
|---|---|---|
| `signalType?` | `FraudSignalType` | Snake-case signal type Types: [`FraudSignalType`](#fraudsignaltype) |
| `action?` | `FraudDecision` | Snake-case decision Types: [`FraudDecision`](#frauddecision) |
| `enabled?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### FraudRuleOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `signalType` | `FraudSignalType` | Snake-case signal type Types: [`FraudSignalType`](#fraudsignaltype) |
| `threshold` | `number` |  |
| `action` | `FraudDecision` | Snake-case decision Types: [`FraudDecision`](#frauddecision) |
| `enabled` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### FraudSignalOutput

| Field | Type | Description |
|---|---|---|
| `orderId` | `string` |  |
| `signalType` | `FraudSignalType` | Snake-case signal type Types: [`FraudSignalType`](#fraudsignaltype) |
| `score` | `number` |  |
| `details` | `string` |  |
| `detectedAt` | `string` |  |

### FraudSignalType

Kind of fraud signal detected on an order (`FraudSignalType`).

```ts
type FraudSignalType = 'velocity_spike' | 'address_mismatch' | 'high_value_first_order' | 'geo_ip_anomaly' | 'bin_country_mismatch' | 'device_fingerprint' | 'proxy_vpn' | 'disposable_email' | 'payment_retries' | 'unusual_time'
```

One of: `'velocity_spike'`, `'address_mismatch'`, `'high_value_first_order'`, `'geo_ip_anomaly'`, `'bin_country_mismatch'`, `'device_fingerprint'`, `'proxy_vpn'`, `'disposable_email'`, `'payment_retries'`, `'unusual_time'`.

### FulfillmentMetricsOutput

| Field | Type | Description |
|---|---|---|
| `avgTimeToShipHours?` | `number` |  |
| `avgTimeToDeliverHours?` | `number` |  |
| `onTimeShippingPercent?` | `number` |  |
| `onTimeDeliveryPercent?` | `number` |  |
| `shippedToday` | `number` |  |
| `awaitingShipment` | `number` |  |

### FulfillmentStatus

Order-level fulfillment status (`OrderOutput.fulfillmentStatus`, `OrderFilterInput.fulfillmentStatus`).

```ts
type FulfillmentStatus = 'unfulfilled' | 'partially_fulfilled' | 'fulfilled' | 'shipped' | 'delivered'
```

One of: `'unfulfilled'`, `'partially_fulfilled'`, `'fulfilled'`, `'shipped'`, `'delivered'`.

### FulfillmentType

How a cart will be fulfilled (`CartOutput.fulfillmentType`; the engine renders pickup as `pick-up`).

```ts
type FulfillmentType = 'shipping' | 'pick-up' | 'digital'
```

One of: `'shipping'`, `'pick-up'`, `'digital'`.

### GiftCardFilterInput

| Field | Type | Description |
|---|---|---|
| `status?` | `GiftCardStatus` | Types: [`GiftCardStatus`](#giftcardstatus) |
| `code?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### GiftCardOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `code` | `string` |  |
| `initialBalance` | `string` | Exact decimal string |
| `currentBalance` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `status` | `GiftCardStatus` | Types: [`GiftCardStatus`](#giftcardstatus) |
| `recipientEmail?` | `string` |  |
| `senderName?` | `string` |  |
| `message?` | `string` |  |
| `expiresAt?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### GiftCardStatus

Gift card status, shared by outputs and inputs (case-insensitive on input).

```ts
type GiftCardStatus = 'active' | 'depleted' | 'expired' | 'disabled'
```

One of: `'active'`, `'depleted'`, `'expired'`, `'disabled'`.

### GiftCardTransactionOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `giftCardId` | `string` |  |
| `amount` | `string` | Exact decimal string |
| `balanceAfter` | `string` | Exact decimal string |
| `transactionType` | `GiftCardTransactionType` | Types: [`GiftCardTransactionType`](#giftcardtransactiontype) |
| `referenceId?` | `string` |  |
| `createdAt` | `string` |  |

### GiftCardTransactionType

Gift card ledger entry kind on `GiftCardTransactionOutput.transactionType`.

```ts
type GiftCardTransactionType = 'charge' | 'refund' | 'adjustment'
```

One of: `'charge'`, `'refund'`, `'adjustment'`.

### GlAccountFilterInput

Optional filters for `GeneralLedger.listAccounts`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `accountType?` | `GlAccountTypeFilter` | The rendered form (`Asset`) or lowercase (`asset`). Types: [`GlAccountTypeFilter`](#glaccounttypefilter) |
| `parentAccountId?` | `string` |  |
| `status?` | `GlAccountStatusInput` | The rendered form (`Active`) or lowercase (`active`). Types: [`GlAccountStatusInput`](#glaccountstatusinput) |
| `isPosting?` | `boolean` |  |
| `isHeader?` | `boolean` |  |
| `search?` | `string` | Matches account number or name. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### GlAccountOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `accountNumber` | `string` |  |
| `name` | `string` |  |
| `accountType` | `GlAccountType` | Types: [`GlAccountType`](#glaccounttype) |
| `balance` | `number` | **Deprecated.** Use the `balanceExact` twin; float money will be removed in 2.0. |
| `balanceExact` | `string` | Exact base-10 balance, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `status` | `GlAccountStatus` | Types: [`GlAccountStatus`](#glaccountstatus) |
| `description?` | `string` |  |

### GlAccountStatus

General-ledger account status as rendered on `GlAccountOutput.status` (Rust `Debug` form).

```ts
type GlAccountStatus = 'Active' | 'Inactive' | 'Archived'
```

One of: `'Active'`, `'Inactive'`, `'Archived'`.

### GlAccountStatusInput

Account status accepted by `GlAccountFilterInput.status`: the rendered form or lowercase (strict).

```ts
type GlAccountStatusInput = GlAccountStatus | 'active' | 'inactive' | 'archived'
```

Types: [`GlAccountStatus`](#glaccountstatus)

### GlAccountType

General-ledger account type as rendered on `GlAccountOutput.accountType` (Rust `Debug` form).

```ts
type GlAccountType = 'Asset' | 'Liability' | 'Equity' | 'Revenue' | 'Expense'
```

One of: `'Asset'`, `'Liability'`, `'Equity'`, `'Revenue'`, `'Expense'`.

### GlAccountTypeFilter

Account type accepted by `GlAccountFilterInput.accountType`: the rendered form or lowercase (strict).

```ts
type GlAccountTypeFilter = GlAccountType | GlAccountTypeInput
```

Types: [`GlAccountType`](#glaccounttype), [`GlAccountTypeInput`](#glaccounttypeinput)

### GlAccountTypeInput

Account type accepted by `CreateGlAccountInput.accountType` (case-insensitive).

```ts
type GlAccountTypeInput = 'asset' | 'liability' | 'equity' | 'revenue' | 'expense'
```

One of: `'asset'`, `'liability'`, `'equity'`, `'revenue'`, `'expense'`.

### GlBalanceSide

Normal balance side on `RevaluationLineOutput.normalBalance`.

```ts
type GlBalanceSide = 'debit' | 'credit' | 'unknown'
```

One of: `'debit'`, `'credit'`, `'unknown'`.

### GlJournalEntryStatus

Journal entry status as rendered on `JournalEntryOutput.status` (Rust `Debug` form).

```ts
type GlJournalEntryStatus = 'Draft' | 'Pending' | 'Posted' | 'Voided' | 'Reversed'
```

One of: `'Draft'`, `'Pending'`, `'Posted'`, `'Voided'`, `'Reversed'`.

### GlJournalEntryStatusInput

Journal entry status accepted by `JournalEntryFilterInput.status`: the rendered form or lowercase (strict).

```ts
type GlJournalEntryStatusInput = GlJournalEntryStatus | 'draft' | 'pending' | 'posted' | 'voided' | 'reversed'
```

Types: [`GlJournalEntryStatus`](#gljournalentrystatus)

### GlPeriodFilterInput

| Field | Type | Description |
|---|---|---|
| `fiscalYear?` | `number` | Filter by fiscal year |
| `status?` | `GlPeriodStatus` | Filter by status: one of `future`, `open`, `closed`, `locked` Types: [`GlPeriodStatus`](#glperiodstatus) |
| `limit?` | `number` | Maximum results |
| `offset?` | `number` | Offset for pagination |

### GlPeriodOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `periodName` | `string` |  |
| `fiscalYear` | `number` |  |
| `periodNumber` | `number` |  |
| `startDate` | `string` | ISO date (YYYY-MM-DD) |
| `endDate` | `string` | ISO date (YYYY-MM-DD) |
| `status` | `GlPeriodStatus` | One of `future`, `open`, `closed`, `locked` Types: [`GlPeriodStatus`](#glperiodstatus) |
| `closedBy?` | `string` |  |

### GlPeriodStatus

Accounting period status (`GlPeriodOutput.status`, `CloseMonthReportOutput.periodStatus`, `GlPeriodFilterInput.status`).

```ts
type GlPeriodStatus = 'future' | 'open' | 'closed' | 'locked'
```

One of: `'future'`, `'open'`, `'closed'`, `'locked'`.

### HealthGrade

Operational health grade derived from a topology snapshot (`HealthGrade`).

```ts
type HealthGrade = 'unknown' | 'healthy' | 'degraded' | 'critical'
```

One of: `'unknown'`, `'healthy'`, `'degraded'`, `'critical'`.

### HybridEncryptionResultOutput

| Field | Type | Description |
|---|---|---|
| `payloadEncryptedJson` | `string` |  |
| `salt` | `Buffer` |  |
| `payloadPlainHash` | `Buffer` |  |
| `payloadCipherHash` | `Buffer` |  |

### HybridPayloadAadParamsInput

| Field | Type | Description |
|---|---|---|
| `vesVersion` | `number` |  |
| `tenantId` | `string` |  |
| `storeId` | `string` |  |
| `eventId` | `string` |  |
| `sourceAgentId` | `string` |  |
| `agentKeyId` | `number` |  |
| `entityType` | `string` |  |
| `entityId` | `string` |  |
| `eventType` | `string` |  |
| `createdAt` | `string` |  |
| `payloadPlainHash` | `Buffer` |  |

### HybridRecipientKeypairOutput

| Field | Type | Description |
|---|---|---|
| `kid` | `number` |  |
| `x25519PublicKey` | `Buffer` |  |
| `x25519PrivateKey` | `Buffer` |  |
| `mlKem768PublicKey` | `Buffer` |  |
| `mlKem768Seed` | `Buffer` |  |

### HybridRecipientPrivateKeyInput

| Field | Type | Description |
|---|---|---|
| `x25519PrivateKey` | `Buffer` |  |
| `mlKem768Seed` | `Buffer` |  |

### HybridRecipientPublicKeyInput

| Field | Type | Description |
|---|---|---|
| `kid` | `number` |  |
| `x25519PublicKey` | `Buffer` |  |
| `mlKem768PublicKey` | `Buffer` |  |

### HybridSignatureBundleOutput

| Field | Type | Description |
|---|---|---|
| `ed25519Signature` | `Buffer` |  |
| `mlDsa65Signature` | `Buffer` |  |

### HybridSigningKeypairOutput

| Field | Type | Description |
|---|---|---|
| `ed25519PublicKey` | `Buffer` |  |
| `ed25519PrivateKey` | `Buffer` |  |
| `mlDsa65PublicKey` | `Buffer` |  |
| `mlDsa65Seed` | `Buffer` |  |

### ImportConflictPolicy

How a structured import treats a record that already exists (`maintenance::ConflictPolicy`); exact, lowercase.

```ts
type ImportConflictPolicy = 'skip' | 'fail'
```

One of: `'skip'`, `'fail'`.

### ImportOptionsInput

Options controlling a structured import.

| Field | Type | Description |
|---|---|---|
| `domains?` | `Array<string>` |  |
| `onConflict?` | `ImportConflictPolicy` | `skip` (default) or `fail`. Types: [`ImportConflictPolicy`](#importconflictpolicy) |
| `dryRun?` | `boolean` |  |

### ImportReportOutput

Result of a structured import.

| Field | Type | Description |
|---|---|---|
| `created` | `Array<DomainCountOutput>` | Types: [`DomainCountOutput`](#domaincountoutput) |
| `skipped` | `Array<DomainCountOutput>` | Types: [`DomainCountOutput`](#domaincountoutput) |
| `unsupportedDomains` | `Array<string>` |  |
| `totalCreated` | `number` |  |

### InboundShipmentFilterInput

| Field | Type | Description |
|---|---|---|
| `supplierId?` | `string` |  |
| `warehouseId?` | `string` |  |
| `status?` | `InboundShipmentStatus` | pending, in_transit, arrived, partially_received, received, cancelled Types: [`InboundShipmentStatus`](#inboundshipmentstatus) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### InboundShipmentItemOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `inboundShipmentId` | `string` |  |
| `productId` | `string` |  |
| `sku` | `string` |  |
| `quantityExpected` | `string` | Exact decimal string |
| `quantityReceived` | `string` | Exact decimal string |

### InboundShipmentOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `number` | `string` |  |
| `supplierId` | `string` |  |
| `purchaseOrderId?` | `string` |  |
| `warehouseId?` | `string` |  |
| `carrier?` | `string` |  |
| `trackingNumber?` | `string` |  |
| `status` | `InboundShipmentStatus` | pending, in_transit, arrived, partially_received, received, cancelled Types: [`InboundShipmentStatus`](#inboundshipmentstatus) |
| `items` | `Array<InboundShipmentItemOutput>` | Types: [`InboundShipmentItemOutput`](#inboundshipmentitemoutput) |
| `expectedAt?` | `string` | RFC 3339 timestamp |
| `receivedAt?` | `string` | RFC 3339 timestamp |
| `notes?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### InboundShipmentStatus

Inbound shipment (ASN) lifecycle status (`InboundShipmentStatus`).

```ts
type InboundShipmentStatus = 'pending' | 'in_transit' | 'arrived' | 'partially_received' | 'received' | 'cancelled'
```

One of: `'pending'`, `'in_transit'`, `'arrived'`, `'partially_received'`, `'received'`, `'cancelled'`.

### IncomeStatementOutput

| Field | Type | Description |
|---|---|---|
| `periodStart` | `string` |  |
| `periodEnd` | `string` |  |
| `totalRevenue` | `number` | **Deprecated.** Use the `totalRevenueExact` twin; float money will be removed in 2.0. |
| `totalRevenueExact` | `string` | Exact base-10 total revenue, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `totalExpenses` | `number` | **Deprecated.** Use the `totalExpensesExact` twin; float money will be removed in 2.0. |
| `totalExpensesExact` | `string` | Exact base-10 total expenses, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `netIncome` | `number` | **Deprecated.** Use the `netIncomeExact` twin; float money will be removed in 2.0. |
| `netIncomeExact` | `string` | Exact base-10 net income, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### IngestLineItemInput

| Field | Type | Description |
|---|---|---|
| `externalSku` | `string` |  |
| `quantity` | `string` | Exact decimal string |
| `productId?` | `string` |  |

### IngestOrderInput

| Field | Type | Description |
|---|---|---|
| `channelId?` | `string` |  |
| `externalOrderId` | `string` |  |
| `externalStatus?` | `string` |  |
| `metadata?` | `string` | JSON string |
| `items` | `Array<IngestLineItemInput>` | Types: [`IngestLineItemInput`](#ingestlineiteminput) |

### InspectionFilterInput

Filter for `quality.listInspections()`

| Field | Type | Description |
|---|---|---|
| `inspectionType?` | `'incoming' \| 'receiving' \| 'in_process' \| 'final' \| 'random' \| 'return'` | Inspection type. |
| `status?` | `'pending' \| 'scheduled' \| 'in_progress' \| 'passed' \| 'failed' \| 'partial_pass' \| 'on_hold' \| 'cancelled'` | Inspection status. |
| `referenceType?` | `string` |  |
| `referenceId?` | `string` |  |
| `inspectorId?` | `string` |  |
| `fromDate?` | `string` | RFC 3339 timestamp (inclusive lower bound on created_at) |
| `toDate?` | `string` | RFC 3339 timestamp (inclusive upper bound on created_at) |
| `limit?` | `number` | Page size (server default 500, hard cap 1000) |
| `offset?` | `number` |  |
| `afterCursor?` | `Array<string>` | Keyset cursor: `[createdAt, id]` |

### InspectionOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `inspectionNumber` | `string` |  |
| `inspectionType` | `InspectionType` | Types: [`InspectionType`](#inspectiontype) |
| `referenceType` | `string` |  |
| `referenceId` | `string` |  |
| `status` | `InspectionStatus` | Types: [`InspectionStatus`](#inspectionstatus) |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### InspectionStatus

Inspection status as rendered on `InspectionOutput.status` (Rust `Debug` form).

```ts
type InspectionStatus = 'Pending' | 'Scheduled' | 'InProgress' | 'Passed' | 'Failed' | 'PartialPass' | 'OnHold' | 'Cancelled'
```

One of: `'Pending'`, `'Scheduled'`, `'InProgress'`, `'Passed'`, `'Failed'`, `'PartialPass'`, `'OnHold'`, `'Cancelled'`.

### InspectionType

Inspection type as rendered on `InspectionOutput.inspectionType` (Rust `Debug` form).

```ts
type InspectionType = 'Incoming' | 'Receiving' | 'InProcess' | 'Final' | 'Random' | 'Return'
```

One of: `'Incoming'`, `'Receiving'`, `'InProcess'`, `'Final'`, `'Random'`, `'Return'`.

Types: [`Receiving`](#commercereceiving)

### InspectionTypeInput

Inspection type accepted by `CreateInspectionInput.inspectionType` (case-insensitive).

```ts
type InspectionTypeInput = 'incoming' | 'receiving' | 'in_process' | 'inprocess' | 'final' | 'random' | 'return'
```

One of: `'incoming'`, `'receiving'`, `'in_process'`, `'inprocess'`, `'final'`, `'random'`, `'return'`.

### IntegrationFieldMappingFilterInput

| Field | Type | Description |
|---|---|---|
| `integrationAccount?` | `string` |  |
| `mappingGroup?` | `string` |  |
| `sourceField?` | `string` |  |
| `isActive?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### IntegrationFieldMappingOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `integrationAccount` | `string` |  |
| `mappingGroup` | `string` |  |
| `sourceField` | `string` |  |
| `destinationField` | `string` |  |
| `template?` | `string` |  |
| `transform` | `FieldTransform` | Snake-case transform Types: [`FieldTransform`](#fieldtransform) |
| `fallback?` | `string` |  |
| `isActive` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### IntegrationMappingFilterInput

| Field | Type | Description |
|---|---|---|
| `integration?` | `string` |  |
| `mappingGroup?` | `string` |  |
| `fieldName?` | `string` |  |
| `isActive?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### IntegrationMappingOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `integration` | `string` |  |
| `mappingGroup` | `string` |  |
| `fieldName` | `string` |  |
| `externalValue` | `string` |  |
| `internalValue` | `string` |  |
| `isActive` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### InventoryHealthOutput

| Field | Type | Description |
|---|---|---|
| `totalSkus` | `number` |  |
| `inStockSkus` | `number` |  |
| `lowStockSkus` | `number` |  |
| `outOfStockSkus` | `number` |  |
| `totalValue` | `number` | **Deprecated.** Use the `totalValueExact` twin; float money will be removed in 2.0. |
| `totalValueExact` | `string` | Exact base-10 total value, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### InventoryItemOutput

| Field | Type | Description |
|---|---|---|
| `id` | `number` |  |
| `sku` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `unitOfMeasure` | `string` |  |
| `isActive` | `boolean` |  |

### InventoryMovementOutput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name` | `string` |  |
| `unitsSold` | `number` |  |
| `unitsReceived` | `number` |  |
| `unitsReturned` | `number` |  |
| `unitsAdjusted` | `number` |  |
| `netChange` | `number` |  |

### InventorySearchResultOutput

| Field | Type | Description |
|---|---|---|
| `item` | `InventoryItemOutput` | Types: [`InventoryItemOutput`](#inventoryitemoutput) |
| `distance` | `number` |  |
| `score` | `number` |  |

### InvoiceFilterInput

Filter for `invoices.list()`

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `orderId?` | `string` |  |
| `status?` | `InvoiceStatus` | Invoice status Types: [`InvoiceStatus`](#invoicestatus) |
| `overdueOnly?` | `boolean` | Only invoices past their due date |
| `fromDate?` | `string` | RFC 3339 timestamp (inclusive lower bound on invoice date) |
| `toDate?` | `string` | RFC 3339 timestamp (inclusive upper bound on invoice date) |
| `dueFrom?` | `string` | RFC 3339 timestamp (inclusive lower bound on due date) |
| `dueTo?` | `string` | RFC 3339 timestamp (inclusive upper bound on due date) |
| `minTotal?` | `string` | Exact decimal string (inclusive lower bound on total) |
| `maxTotal?` | `string` | Exact decimal string (inclusive upper bound on total) |
| `minBalance?` | `string` | Exact decimal string (inclusive lower bound on balance due) |
| `invoiceNumber?` | `string` | Search by invoice number |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |

### InvoiceOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `invoiceNumber` | `string` |  |
| `customerId` | `string` |  |
| `orderId?` | `string` |  |
| `status` | `InvoiceStatus` | Types: [`InvoiceStatus`](#invoicestatus) |
| `subtotal` | `number` | **Deprecated.** Use the `subtotalExact` twin; float money will be removed in 2.0. |
| `subtotalExact` | `string` | Exact base-10 subtotal, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `taxAmount` | `number` | **Deprecated.** Use the `taxAmountExact` twin; float money will be removed in 2.0. |
| `taxAmountExact` | `string` | Exact base-10 tax amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `total` | `number` | **Deprecated.** Use the `totalExact` twin; float money will be removed in 2.0. |
| `totalExact` | `string` | Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `amountPaid` | `number` | **Deprecated.** Use the `amountPaidExact` twin; float money will be removed in 2.0. |
| `amountPaidExact` | `string` | Exact base-10 amount paid, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `dueDate` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### InvoiceStatus

Invoice status (`InvoiceOutput.status`, `InvoiceFilterInput.status`).

```ts
type InvoiceStatus = 'draft' | 'sent' | 'viewed' | 'partially_paid' | 'paid' | 'overdue' | 'voided' | 'written_off' | 'disputed'
```

One of: `'draft'`, `'sent'`, `'viewed'`, `'partially_paid'`, `'paid'`, `'overdue'`, `'voided'`, `'written_off'`, `'disputed'`.

### ItemCostFilterInput

Optional filters for `CostAccounting.listItemCosts`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `sku?` | `string` |  |
| `costMethod?` | `CostMethodFilter` | The rendered form (`Fifo`) or the engine's lowercase (`fifo`). Types: [`CostMethodFilter`](#costmethodfilter) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### ItemCostOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `sku` | `string` |  |
| `costMethod` | `CostMethod` | Types: [`CostMethod`](#costmethod) |
| `standardCost` | `number` | **Deprecated.** Use the `standardCostExact` twin; float money will be removed in 2.0. |
| `standardCostExact` | `string` | Exact base-10 standard cost, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `averageCost` | `number` | **Deprecated.** Use the `averageCostExact` twin; float money will be removed in 2.0. |
| `averageCostExact` | `string` | Exact base-10 average cost, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `lastCost` | `number` | **Deprecated.** Use the `lastCostExact` twin; float money will be removed in 2.0. |
| `lastCostExact` | `string` | Exact base-10 last cost, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `materialCost` | `number` | **Deprecated.** Use the `materialCostExact` twin; float money will be removed in 2.0. |
| `materialCostExact` | `string` | Exact base-10 material cost, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `laborCost` | `number` | **Deprecated.** Use the `laborCostExact` twin; float money will be removed in 2.0. |
| `laborCostExact` | `string` | Exact base-10 labor cost, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `overheadCost` | `number` | **Deprecated.** Use the `overheadCostExact` twin; float money will be removed in 2.0. |
| `overheadCostExact` | `string` | Exact base-10 overhead cost, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### JournalEntryFilterInput

Optional filters for `GeneralLedger.listJournalEntries`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `periodId?` | `string` |  |
| `status?` | `GlJournalEntryStatusInput` | The rendered form (`Posted`) or lowercase (`posted`). Types: [`GlJournalEntryStatusInput`](#gljournalentrystatusinput) |
| `accountId?` | `string` |  |
| `fromDate?` | `string` | `YYYY-MM-DD`. |
| `toDate?` | `string` | `YYYY-MM-DD`. |
| `sourceDocumentType?` | `string` |  |
| `sourceDocumentId?` | `string` |  |
| `search?` | `string` | Matches entry number, memo or reference. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### JournalEntryOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `entryNumber` | `string` |  |
| `entryDate` | `string` |  |
| `description` | `string` |  |
| `status` | `GlJournalEntryStatus` | Types: [`GlJournalEntryStatus`](#gljournalentrystatus) |
| `createdAt` | `string` |  |

### JurisdictionFilterInput

| Field | Type | Description |
|---|---|---|
| `countryCode?` | `string` |  |
| `stateCode?` | `string` |  |
| `level?` | `TaxJurisdictionLevel` | Types: [`TaxJurisdictionLevel`](#taxjurisdictionlevel) |
| `activeOnly?` | `boolean` |  |

### JurisdictionSummaryOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `code` | `string` |  |
| `level` | `TaxJurisdictionLevel` | Types: [`TaxJurisdictionLevel`](#taxjurisdictionlevel) |
| `totalRate` | `number` |  |
| `totalTax` | `number` | **Deprecated.** Use the `totalTaxExact` twin; float money will be removed in 2.0. |
| `totalTaxExact` | `string` | Exact base-10 total tax, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### KernelA2AEscrowRefPayload

`a2a.escrow.fund` / `a2a.escrow.release` (`FundA2AEscrow`, `ReleaseA2AEscrow`).

| Field | Type | Description |
|---|---|---|
| `escrow_id` | `string` |  |

### KernelApprovalEvidence

Evidence for an approval required by policy (`ApprovalEvidence`).

| Field | Type | Description |
|---|---|---|
| `approval_id` | `string` |  |
| `approved_by` | `string` |  |
| `scope` | `string` | Must equal the command's `command_type`. |
| `tenant_id?` | `string \| null` |  |
| `store_id?` | `string \| null` |  |
| `idempotency_key?` | `string \| null` | Must equal the command's `idempotency_key`. |
| `approved_at` | `string` |  |
| `expires_at?` | `string \| null` |  |

### KernelAuthorityEvidence

Proof that a trusted issuer signed the semantic command
(`AuthorityEvidence`). `signature` is hex-encoded Ed25519 over the
canonical command hash; `key_id` must appear in
`KernelPolicy.trusted_authority_keys`.

| Field | Type | Description |
|---|---|---|
| `issuer` | `string` |  |
| `key_id` | `string` |  |
| `issued_at` | `string` |  |
| `expires_at` | `string` |  |
| `signature` | `string` |  |

### KernelChargeSubscriptionPayload

`subscriptions.charge` (`ChargeSubscription`).

| Field | Type | Description |
|---|---|---|
| `billing_cycle_id` | `string` |  |
| `payment_method` | `KernelPaymentMethodType` | Types: [`KernelPaymentMethodType`](#kernelpaymentmethodtype) |
| `processor?` | `string \| null` |  |

### KernelCommand

A governed command: the envelope discriminated by `command_type`, each
variant carrying its own payload shape.

```ts
type KernelCommand = KernelCommandEnvelope<'inventory.item.create', KernelCreateInventoryItemPayload> | KernelCommandEnvelope<'products.create', KernelCreateProductPayload> | KernelCommandEnvelope<'payments.create', KernelCreatePaymentPayload> | KernelCommandEnvelope<'payments.create_refund', KernelCreateRefundPayload> | KernelCommandEnvelope<'inventory.reserve', KernelReserveInventoryPayload> | KernelCommandEnvelope<'inventory.reservation.confirm', KernelConfirmInventoryReservationPayload> | KernelCommandEnvelope<'inventory.reservation.release', KernelReleaseInventoryReservationPayload> | KernelCommandEnvelope<'orders.transition', KernelTransitionOrderPayload> | KernelCommandEnvelope<'orders.ship', KernelShipOrderPayload> | KernelCommandEnvelope<'returns.transition', KernelTransitionReturnPayload> | KernelCommandEnvelope<'ledger.post', KernelPostJournalEntryPayload> | KernelCommandEnvelope<'x402.settle', KernelSettleX402IntentPayload> | KernelCommandEnvelope<'checkout.commit', KernelCommitCheckoutPayload> | KernelCommandEnvelope<'subscriptions.charge', KernelChargeSubscriptionPayload> | KernelCommandEnvelope<'a2a.escrow.create', KernelCreateA2AEscrowPayload> | KernelCommandEnvelope<'a2a.escrow.dispute', KernelDisputeA2AEscrowPayload> | KernelCommandEnvelope<'a2a.escrow.fund', KernelA2AEscrowRefPayload> | KernelCommandEnvelope<'a2a.escrow.release', KernelA2AEscrowRefPayload> | KernelCommandEnvelope<'a2a.escrow.refund', KernelRefundA2AEscrowPayload> | KernelCommandEnvelope<'a2a.dispute.file', KernelFileA2ADisputePayload> | KernelCommandEnvelope<'a2a.dispute.evidence.submit', KernelSubmitA2ADisputeEvidencePayload> | KernelCommandEnvelope<'a2a.dispute.resolve', KernelResolveA2ADisputePayload>
```

Types: [`KernelCommandEnvelope`](#kernelcommandenvelope), [`KernelCreateInventoryItemPayload`](#kernelcreateinventoryitempayload), [`KernelCreateProductPayload`](#kernelcreateproductpayload), [`KernelCreatePaymentPayload`](#kernelcreatepaymentpayload), [`KernelCreateRefundPayload`](#kernelcreaterefundpayload), [`KernelReserveInventoryPayload`](#kernelreserveinventorypayload), [`KernelConfirmInventoryReservationPayload`](#kernelconfirminventoryreservationpayload), [`KernelReleaseInventoryReservationPayload`](#kernelreleaseinventoryreservationpayload), [`KernelTransitionOrderPayload`](#kerneltransitionorderpayload), [`KernelShipOrderPayload`](#kernelshiporderpayload), [`KernelTransitionReturnPayload`](#kerneltransitionreturnpayload), [`KernelPostJournalEntryPayload`](#kernelpostjournalentrypayload), [`KernelSettleX402IntentPayload`](#kernelsettlex402intentpayload), [`KernelCommitCheckoutPayload`](#kernelcommitcheckoutpayload), [`KernelChargeSubscriptionPayload`](#kernelchargesubscriptionpayload), [`KernelCreateA2AEscrowPayload`](#kernelcreatea2aescrowpayload), [`KernelDisputeA2AEscrowPayload`](#kerneldisputea2aescrowpayload), [`KernelA2AEscrowRefPayload`](#kernela2aescrowrefpayload), [`KernelRefundA2AEscrowPayload`](#kernelrefunda2aescrowpayload), [`KernelFileA2ADisputePayload`](#kernelfilea2adisputepayload), [`KernelSubmitA2ADisputeEvidencePayload`](#kernelsubmita2adisputeevidencepayload), [`KernelResolveA2ADisputePayload`](#kernelresolvea2adisputepayload)

### KernelCommandEnvelope

Type parameters `<TType extends string, TPayload>`.

Versioned execution request shared by every governed command
(`CommandEnvelope<T>`). `command_type` selects the payload shape; see
`KernelCommand` for the closed catalog.

| Field | Type | Description |
|---|---|---|
| `contract_version` | `string` | Wire-contract version; currently `"1.0"`. |
| `command_id` | `string` | Unique, non-nil UUID for this invocation. |
| `idempotency_key` | `string` | Stable retry key. Retries must reuse this value. |
| `command_type` | `TType` |  |
| `principal` | `KernelPrincipal` | Types: [`KernelPrincipal`](#kernelprincipal) |
| `store_id?` | `string \| null` | Logical store boundary; required by default policy (`requires_store`). |
| `correlation_id?` | `string \| null` | Root workflow identifier (UUID). |
| `causation_id?` | `string \| null` | Command or event that caused this command (UUID). |
| `expected_version?` | `number \| null` | Optimistic concurrency version expected by the caller. |
| `policy_version?` | `string \| null` | Policy revision the caller expects; a mismatch is rejected. |
| `approval?` | `KernelApprovalEvidence \| null` | Types: [`KernelApprovalEvidence`](#kernelapprovalevidence) |
| `authority?` | `KernelAuthorityEvidence \| null` | Types: [`KernelAuthorityEvidence`](#kernelauthorityevidence) |
| `mandate?` | `EconomicMandate \| null` | Types: [`EconomicMandate`](#economicmandate) |
| `commitment?` | `EconomicCommitment \| null` | Types: [`EconomicCommitment`](#economiccommitment) |
| `deadline?` | `string \| null` | Time after which execution should not begin. |
| `trace_id?` | `string \| null` |  |
| `mode` | `KernelExecutionMode` | Types: [`KernelExecutionMode`](#kernelexecutionmode) |
| `payload` | `TPayload` |  |
| `issued_at` | `string` |  |

### KernelCommandPolicy

Policy requirements for one namespaced command (`KernelCommandPolicy`).
Every field is serde-defaulted. Note the defaults that are `true`:
`requires_tenant`, `requires_store` and `requires_agent_delegation`.

| Field | Type | Description |
|---|---|---|
| `required_capabilities?` | `string[]` | Capabilities the principal must hold. Default: none. |
| `requires_approval?` | `boolean` | Default `false`. |
| `requires_tenant?` | `boolean` | Default `true`. |
| `requires_store?` | `boolean` | Default `true`. |
| `allowed_tenant_ids?` | `string[]` | Empty permits any non-empty tenant when `requires_tenant` is on. |
| `allowed_store_ids?` | `string[]` | Empty permits any non-empty store when `requires_store` is on. |
| `requires_agent_delegation?` | `boolean` | Default `true`: agents must name `delegated_by`. |
| `requires_signed_authority?` | `boolean` | Default `false`. |
| `requires_mandate?` | `boolean` | Default `false`. |
| `requires_budget?` | `boolean` | Default `false`: require `commitment.budget_id` on money-moving commands. |
| `max_amount?` | `MoneyWire \| null` | Maximum exact fiat commitment for one command. Types: [`MoneyWire`](#moneywire) |
| `approval_above?` | `MoneyWire \| null` | Require approval only above this exact fiat amount. Types: [`MoneyWire`](#moneywire) |
| `max_asset_amount?` | `AssetAmountWire \| null` | Types: [`AssetAmountWire`](#assetamountwire) |
| `approval_above_asset?` | `AssetAmountWire \| null` | Types: [`AssetAmountWire`](#assetamountwire) |
| `max_quantity?` | `string \| null` | Maximum exact unit commitment, as a decimal string. |
| `allowed_counterparty_ids?` | `string[]` | Empty permits any counterparty. |

### KernelCommandType

The closed catalog of governed commands `executeKernelCommand` dispatches
(`stateset_embedded::Commerce::execute_kernel_command`). Any other
`command_type` is rejected before a repository is reached.

```ts
type KernelCommandType = KernelCommand['command_type']
```

Types: [`KernelCommand`](#kernelcommand)

### KernelCommitCheckoutPayload

`checkout.commit` (`CommitCheckout`).

| Field | Type | Description |
|---|---|---|
| `cart_id` | `string` |  |
| `stock_policy?` | `KernelStockPolicy \| null` | Omission preserves historical backorder behaviour. Types: [`KernelStockPolicy`](#kernelstockpolicy) |
| `expected_cart_fingerprint?` | `string \| null` | The `fingerprint` from `checkoutSnapshot`; checked under the checkout transaction. |

### KernelConfirmInventoryReservationPayload

`inventory.reservation.confirm` (`ConfirmInventoryReservation`).

| Field | Type | Description |
|---|---|---|
| `reservation_id` | `string` |  |
| `quantity?` | `string \| null` | Omit to confirm the full remaining reservation. |

### KernelCreateA2AEscrowPayload

`a2a.escrow.create` (`CreateA2AEscrow`).

| Field | Type | Description |
|---|---|---|
| `quote_id?` | `string \| null` |  |
| `payment_id?` | `string \| null` |  |
| `buyer_address` | `string` |  |
| `seller_address` | `string` |  |
| `amount` | `string` |  |
| `asset` | `string` |  |
| `network` | `string` |  |
| `release_conditions?` | `Array<Record<string, unknown>>` |  |
| `expires_at` | `string` |  |
| `auto_release_after?` | `string \| null` |  |
| `metadata?` | `Record<string, unknown> \| null` |  |

### KernelCreateInventoryItemPayload

`inventory.item.create` (`CreateInventoryItem`).

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name` | `string` |  |
| `description?` | `string \| null` |  |
| `unit_of_measure?` | `string \| null` |  |
| `initial_quantity?` | `string \| null` |  |
| `location_id?` | `number \| null` |  |
| `reorder_point?` | `string \| null` |  |
| `safety_stock?` | `string \| null` |  |

### KernelCreatePaymentPayload

`payments.create` (`CreatePayment`). Every field is serde-defaulted, so all
are optional on the wire; a real payment still needs `amount` and
`payment_method`. `metadata` is a free-form string here, not an object.

| Field | Type | Description |
|---|---|---|
| `order_id?` | `string \| null` |  |
| `invoice_id?` | `string \| null` |  |
| `customer_id?` | `string \| null` |  |
| `payment_method?` | `KernelPaymentMethodType` | Types: [`KernelPaymentMethodType`](#kernelpaymentmethodtype) |
| `amount?` | `string` |  |
| `currency?` | `string \| null` |  |
| `external_id?` | `string \| null` |  |
| `idempotency_key?` | `string \| null` |  |
| `processor?` | `string \| null` |  |
| `card_brand?` | `'unknown' \| 'visa' \| 'mastercard' \| 'amex' \| 'discover' \| 'diners_club' \| 'jcb' \| 'union_pay' \| null` |  |
| `card_last4?` | `string \| null` |  |
| `card_exp_month?` | `number \| null` |  |
| `card_exp_year?` | `number \| null` |  |
| `blockchain_network?` | `'solana' \| 'solana_devnet' \| 'set_chain' \| 'set_chain_testnet' \| 'ethereum' \| 'base' \| 'arbitrum' \| 'near' \| 'cosmos' \| null` |  |
| `stablecoin_type?` | `'usdc' \| 'usdt' \| 'ss_usd' \| 'wss_usd' \| 'dai' \| null` |  |
| `from_wallet_address?` | `string \| null` |  |
| `to_wallet_address?` | `string \| null` |  |
| `token_address?` | `string \| null` |  |
| `billing_email?` | `string \| null` |  |
| `billing_name?` | `string \| null` |  |
| `billing_address?` | `string \| null` |  |
| `description?` | `string \| null` |  |
| `metadata?` | `string \| null` |  |

### KernelCreateProductPayload

`products.create` (`CreateProduct`).

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `slug?` | `string \| null` |  |
| `description?` | `string \| null` |  |
| `product_type?` | `'simple' \| 'variable' \| 'bundle' \| 'digital' \| null` |  |
| `attributes?` | `KernelProductAttribute[] \| null` | Types: [`KernelProductAttribute`](#kernelproductattribute) |
| `seo?` | `{ title?: string \| null; description?: string \| null; keywords: string[] } \| null` |  |
| `variants?` | `KernelCreateProductVariantPayload[] \| null` | Types: [`KernelCreateProductVariantPayload`](#kernelcreateproductvariantpayload) |

### KernelCreateProductVariantPayload

Variant input nested in `products.create` (`CreateProductVariant`).

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name?` | `string \| null` |  |
| `price` | `string` |  |
| `compare_at_price?` | `string \| null` |  |
| `cost?` | `string \| null` |  |
| `barcode?` | `string \| null` |  |
| `weight?` | `string \| null` |  |
| `weight_unit?` | `string \| null` |  |
| `options?` | `KernelVariantOption[] \| null` | Types: [`KernelVariantOption`](#kernelvariantoption) |
| `is_default?` | `boolean \| null` |  |

### KernelCreateRefundPayload

`payments.create_refund` (`CreateRefund`). `payment_id` is required in practice.

| Field | Type | Description |
|---|---|---|
| `payment_id` | `string` |  |
| `amount?` | `string \| null` | Defaults to the full payment amount. |
| `reason?` | `string \| null` |  |
| `external_id?` | `string \| null` |  |
| `idempotency_key?` | `string \| null` |  |
| `notes?` | `string \| null` |  |

### KernelDisputeA2AEscrowPayload

`a2a.escrow.dispute` (`DisputeA2AEscrow`).

| Field | Type | Description |
|---|---|---|
| `escrow_id` | `string` |  |
| `reason` | `string` |  |
| `category?` | `string \| null` |  |

### KernelEconomicReceiptContext

Accountability data copied from the command into its durable receipt
(`EconomicReceiptContext`).

| Field | Type | Description |
|---|---|---|
| `principal` | `KernelPrincipal` | Types: [`KernelPrincipal`](#kernelprincipal) |
| `store_id` | `string \| null` |  |
| `correlation_id` | `string \| null` |  |
| `mandate` | `EconomicMandate \| null` | Types: [`EconomicMandate`](#economicmandate) |
| `commitment` | `EconomicCommitment \| null` | Types: [`EconomicCommitment`](#economiccommitment) |
| `approval_id` | `string \| null` |  |
| `authority_issuer` | `string \| null` |  |

### KernelExecutionMode

Non-mutating preview (the safe default) or an authorized mutation.

```ts
type KernelExecutionMode = 'preview' | 'apply'
```

One of: `'preview'`, `'apply'`.

### KernelExecutionStatus

Outcome category recorded in a receipt (`ExecutionStatus`).

```ts
type KernelExecutionStatus = 'previewed' | 'succeeded' | 'rejected' | 'failed'
```

One of: `'previewed'`, `'succeeded'`, `'rejected'`, `'failed'`.

### KernelFileA2ADisputePayload

`a2a.dispute.file` (`FileA2ADispute`).

| Field | Type | Description |
|---|---|---|
| `escrow_id` | `string` |  |
| `claimant_address` | `string` | Must be the escrow buyer or seller; the respondent is derived. |
| `reason` | `string` |  |
| `category` | `string` |  |
| `evidence_deadline` | `string` |  |
| `review_deadline` | `string` |  |
| `metadata?` | `Record<string, unknown> \| null` |  |

### KernelOrderPaymentStatus

Order payment status (`PaymentStatus`).

```ts
type KernelOrderPaymentStatus = 'pending' | 'authorized' | 'paid' | 'partially_paid' | 'refunded' | 'partially_refunded' | 'failed'
```

One of: `'pending'`, `'authorized'`, `'paid'`, `'partially_paid'`, `'refunded'`, `'partially_refunded'`, `'failed'`.

### KernelOrderStatus

Order lifecycle status (`OrderStatus`).

```ts
type KernelOrderStatus = 'pending' | 'confirmed' | 'processing' | 'partially_shipped' | 'shipped' | 'delivered' | 'cancelled' | 'refunded'
```

One of: `'pending'`, `'confirmed'`, `'processing'`, `'partially_shipped'`, `'shipped'`, `'delivered'`, `'cancelled'`, `'refunded'`.

### KernelPaymentMethodType

Payment instrument class (`PaymentMethodType`).

```ts
type KernelPaymentMethodType = 'credit_card' | 'debit_card' | 'bank_transfer' | 'pay_pal' | 'apple_pay' | 'google_pay' | 'crypto' | 'stablecoin' | 'store_credit' | 'gift_card' | 'cash_on_delivery' | 'invoice' | 'other'
```

One of: `'credit_card'`, `'debit_card'`, `'bank_transfer'`, `'pay_pal'`, `'apple_pay'`, `'google_pay'`, `'crypto'`, `'stablecoin'`, `'store_credit'`, `'gift_card'`, `'cash_on_delivery'`, `'invoice'`, `'other'`.

### KernelPolicy

Deterministic, versioned allow-list evaluated before execution
(`KernelPolicy`). Deny by default: a `command_type` absent from `commands`
is rejected. Comes from trusted host configuration, never from model output.

| Field | Type | Description |
|---|---|---|
| `version` | `string` | Stable revision id recorded in every receipt's policy decision. |
| `commands?` | `Record<string, KernelCommandPolicy>` | Rules keyed by `command_type`. Types: [`KernelCommandPolicy`](#kernelcommandpolicy) |
| `trusted_authority_keys?` | `Record<string, string>` | Trusted Ed25519 verifying keys, hex encoded, keyed by `key_id`. |

### KernelPolicyDecision

Policy decision captured with a receipt (`PolicyDecisionEvidence`).

| Field | Type | Description |
|---|---|---|
| `policy_version` | `string` |  |
| `decision_id` | `string` |  |
| `allowed` | `boolean` |  |
| `reason_codes` | `string[]` | Stable codes such as `"policy.capability_missing:payments.create"`. |

### KernelPostJournalEntryPayload

`ledger.post` (`PostJournalEntry`).

| Field | Type | Description |
|---|---|---|
| `journal_entry_id` | `string` |  |
| `posted_by` | `string` |  |

### KernelPrincipal

Authenticated identity and delegation context (`KernelPrincipal`). Comes
from trusted host configuration, never from model-generated arguments.

| Field | Type | Description |
|---|---|---|
| `id` | `string` | Stable subject identifier, e.g. `"agent:buyer-7"`. |
| `kind` | `KernelPrincipalKind` | Types: [`KernelPrincipalKind`](#kernelprincipalkind) |
| `tenant_id?` | `string \| null` | Tenant boundary; required by default policy (`requires_tenant`). |
| `delegated_by?` | `string \| null` | Principal that delegated authority; required for agents by default. |
| `capabilities?` | `string[]` | Capabilities the caller asserts; policy checks them. |

### KernelPrincipalKind

Identity class responsible for a command (`PrincipalKind`).

```ts
type KernelPrincipalKind = 'human' | 'agent' | 'system' | 'integration'
```

One of: `'human'`, `'agent'`, `'system'`, `'integration'`.

### KernelProductAttribute

Product attribute as the engine stores it (`ProductAttribute`).

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `value` | `string` |  |
| `group?` | `string \| null` |  |
| `is_visible` | `boolean` |  |
| `is_variation` | `boolean` |  |

### KernelReceipt

Durable, machine-readable outcome of a governed command
(`ExecutionReceipt<T>`). `result` is the applied domain aggregate as the
engine serialises it (snake_case, exact decimal strings); it depends on the
command type (a `Payment` for `payments.create`, an `Order` for
`orders.ship`, ...) and is `null` for previews with no aggregate yet, and
for rejections.

| Field | Type | Description |
|---|---|---|
| `contract_version` | `string` |  |
| `receipt_id` | `string` |  |
| `command_id` | `string` |  |
| `idempotency_key` | `string` |  |
| `command_type` | `KernelCommandType` | Types: [`KernelCommandType`](#kernelcommandtype) |
| `status` | `KernelExecutionStatus` | Types: [`KernelExecutionStatus`](#kernelexecutionstatus) |
| `result` | `Record<string, unknown> \| null` |  |
| `error_code` | `string \| null` | Stable code such as `"kernel.commitment_amount_mismatch"`; never parse prose. |
| `error_message` | `string \| null` |  |
| `retry` | `KernelRetryDisposition` | Types: [`KernelRetryDisposition`](#kernelretrydisposition) |
| `aggregate_type` | `string \| null` | Affected aggregate category, e.g. `"payment"`. |
| `aggregate_id` | `string \| null` |  |
| `version_before` | `number \| null` |  |
| `version_after` | `number \| null` |  |
| `event_ids` | `string[]` | Events committed atomically with the mutation (UUIDs). |
| `policy` | `KernelPolicyDecision \| null` | Types: [`KernelPolicyDecision`](#kernelpolicydecision) |
| `economic_context?` | `KernelEconomicReceiptContext` | Present on every receipt the executor issues; absent only on hand-built ones. Types: [`KernelEconomicReceiptContext`](#kerneleconomicreceiptcontext) |
| `audit_hash` | `string \| null` | Hex SHA-256 link into the sealed receipt audit chain; set once sealed. |
| `started_at` | `string` |  |
| `completed_at` | `string` |  |

### KernelRefundA2AEscrowPayload

`a2a.escrow.refund` (`RefundA2AEscrow`).

| Field | Type | Description |
|---|---|---|
| `escrow_id` | `string` |  |
| `reason?` | `string \| null` |  |

### KernelReleaseInventoryReservationPayload

`inventory.reservation.release` (`ReleaseInventoryReservation`).

| Field | Type | Description |
|---|---|---|
| `reservation_id` | `string` |  |

### KernelReserveInventoryPayload

`inventory.reserve` (`ReserveInventory`).

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `location_id?` | `number \| null` |  |
| `quantity` | `string` |  |
| `reference_type` | `string` |  |
| `reference_id` | `string` |  |
| `expires_in_seconds?` | `number \| null` |  |

### KernelResolveA2ADisputePayload

`a2a.dispute.resolve` (`ResolveA2ADispute`).

| Field | Type | Description |
|---|---|---|
| `dispute_id` | `string` |  |
| `resolution_type` | `'full_refund' \| 'release_to_seller' \| 'split' \| 'escalated'` |  |
| `buyer_amount?` | `string \| null` | Required for `split`; forbidden otherwise. |
| `seller_amount?` | `string \| null` | Required for `split`; forbidden otherwise. |
| `note?` | `string \| null` |  |

### KernelRetryDisposition

Machine-readable retry guidance (`RetryDisposition`).

```ts
type KernelRetryDisposition = 'never' | 'same_key' | 'after_conflict' | 'after_delay'
```

One of: `'never'`, `'same_key'`, `'after_conflict'`, `'after_delay'`.

### KernelSettleX402IntentPayload

`x402.settle` (`SettleX402Intent`).

| Field | Type | Description |
|---|---|---|
| `intent_id` | `string` |  |
| `tx_hash` | `string` |  |
| `block_number` | `number` |  |

### KernelShipOrderPayload

`orders.ship` (`ShipOrderCommand`).

| Field | Type | Description |
|---|---|---|
| `order_id` | `string` |  |
| `tracking_number?` | `string \| null` |  |
| `lines?` | `KernelShipmentLine[] \| null` | Omit to ship every remaining unit. Types: [`KernelShipmentLine`](#kernelshipmentline) |

### KernelShipmentLine

One order line in a shipment (`ShipmentLineInput`).

| Field | Type | Description |
|---|---|---|
| `order_item_id` | `string` |  |
| `quantity` | `number` |  |

### KernelStockPolicy

What checkout does when a tracked SKU cannot be fully reserved (`StockPolicy`).

```ts
type KernelStockPolicy = 'allow_backorder' | 'reject_if_insufficient'
```

One of: `'allow_backorder'`, `'reject_if_insufficient'`.

### KernelSubmitA2ADisputeEvidencePayload

`a2a.dispute.evidence.submit` (`SubmitA2ADisputeEvidence`).

| Field | Type | Description |
|---|---|---|
| `dispute_id` | `string` |  |
| `submitted_by` | `string` |  |
| `evidence_type` | `string` |  |
| `title` | `string` |  |
| `description?` | `string \| null` |  |
| `content` | `string` |  |

### KernelTransitionOrderPayload

`orders.transition` (`TransitionOrder`).

| Field | Type | Description |
|---|---|---|
| `order_id` | `string` |  |
| `status` | `KernelOrderStatus` | Types: [`KernelOrderStatus`](#kernelorderstatus) |
| `payment_status?` | `KernelOrderPaymentStatus \| null` | Types: [`KernelOrderPaymentStatus`](#kernelorderpaymentstatus) |
| `void_payments?` | `boolean` | Void in-flight payments atomically with a cancel. Omit (or `false`) to have a cancel rejected while captured money is outstanding. |

### KernelTransitionReturnPayload

`returns.transition` (`TransitionReturn`).

| Field | Type | Description |
|---|---|---|
| `return_id` | `string` |  |
| `status` | `'requested' \| 'approved' \| 'rejected' \| 'in_transit' \| 'received' \| 'inspecting' \| 'completed' \| 'cancelled'` |  |

### KernelVariantOption

Variant option pair (`VariantOption`).

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `value` | `string` |  |

### LineItemTaxOutput

| Field | Type | Description |
|---|---|---|
| `lineItemId` | `string` |  |
| `taxableAmount` | `number` | **Deprecated.** Use the `taxableAmountExact` twin; float money will be removed in 2.0. |
| `taxableAmountExact` | `string` | Exact base-10 taxable amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `taxAmount` | `number` | **Deprecated.** Use the `taxAmountExact` twin; float money will be removed in 2.0. |
| `taxAmountExact` | `string` | Exact base-10 tax amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `effectiveRate` | `number` |  |
| `isExempt` | `boolean` |  |
| `exemptionReason?` | `string` |  |
| `taxDetails` | `Array<TaxDetailOutput>` | Types: [`TaxDetailOutput`](#taxdetailoutput) |

### LocationOutput

| Field | Type | Description |
|---|---|---|
| `id` | `number` |  |
| `warehouseId` | `number` |  |
| `code` | `string` |  |
| `locationType` | `WarehouseLocationType` | Types: [`WarehouseLocationType`](#warehouselocationtype) |
| `zone?` | `string` |  |
| `aisle?` | `string` |  |
| `rack?` | `string` |  |
| `bin?` | `string` |  |
| `isActive` | `boolean` |  |
| `isPickable` | `boolean` |  |
| `isReceivable` | `boolean` |  |

### LotFilterInput

Optional filters for `Lots.list`. Every field is optional; an empty object
(or no argument) lists everything, unpaginated, as before.

| Field | Type | Description |
|---|---|---|
| `sku?` | `string` |  |
| `lotNumber?` | `string` |  |
| `status?` | `LotStatusInput` | The rendered form (`OnHold`) or the engine's snake_case (`on_hold`). Types: [`LotStatusInput`](#lotstatusinput) |
| `supplierId?` | `string` |  |
| `workOrderId?` | `string` |  |
| `purchaseOrderId?` | `string` |  |
| `expiringBefore?` | `string` | RFC 3339 timestamp. |
| `expiringAfter?` | `string` | RFC 3339 timestamp. |
| `hasQuantity?` | `boolean` |  |
| `locationId?` | `number` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### LotOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `lotNumber` | `string` |  |
| `sku` | `string` |  |
| `quantityProduced` | `number` |  |
| `quantityAvailable` | `number` |  |
| `quantityReserved` | `number` |  |
| `status` | `LotStatus` | Types: [`LotStatus`](#lotstatus) |
| `productionDate?` | `string` |  |
| `expirationDate?` | `string` |  |
| `createdAt` | `string` |  |

### LotStatus

Lot status as rendered on `LotOutput.status` (Rust `Debug` form).

```ts
type LotStatus = 'Active' | 'Quarantine' | 'Expired' | 'Consumed' | 'OnHold' | 'Recalled' | 'Scrapped'
```

One of: `'Active'`, `'Quarantine'`, `'Expired'`, `'Consumed'`, `'OnHold'`, `'Recalled'`, `'Scrapped'`.

### LotStatusInput

Lot status accepted by `LotFilterInput.status`: the rendered form or the engine's snake_case.

```ts
type LotStatusInput = LotStatus | 'active' | 'quarantine' | 'expired' | 'consumed' | 'on_hold' | 'recalled' | 'scrapped'
```

Types: [`LotStatus`](#lotstatus)

### LowStockItemOutput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name` | `string` |  |
| `onHand` | `number` |  |
| `allocated` | `number` |  |
| `available` | `number` |  |
| `reorderPoint?` | `number` |  |
| `averageDailySales?` | `number` |  |
| `daysOfStock?` | `number` |  |

### LoyaltyAccountFilterInput

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `programId?` | `string` |  |
| `tier?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### LoyaltyAccountOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `customerId` | `string` |  |
| `programId` | `string` |  |
| `pointsBalance` | `number` |  |
| `lifetimePoints` | `number` |  |
| `tier` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### LoyaltyProgramOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `pointsPerDollar` | `number` |  |
| `tiers` | `Array<LoyaltyTierOutput>` | Types: [`LoyaltyTierOutput`](#loyaltytieroutput) |
| `status` | `LoyaltyProgramStatus` | Types: [`LoyaltyProgramStatus`](#loyaltyprogramstatus) |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### LoyaltyProgramStatus

Loyalty program status on `LoyaltyProgramOutput.status`.

```ts
type LoyaltyProgramStatus = 'active' | 'paused' | 'archived'
```

One of: `'active'`, `'paused'`, `'archived'`.

### LoyaltyRewardType

Loyalty reward kind, shared by outputs and inputs (case-insensitive on input).

```ts
type LoyaltyRewardType = 'discount' | 'free_shipping' | 'free_product' | 'store_credit' | 'exclusive_access'
```

One of: `'discount'`, `'free_shipping'`, `'free_product'`, `'store_credit'`, `'exclusive_access'`.

### LoyaltyTierInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `minPoints` | `number` |  |
| `multiplier` | `number` |  |
| `perks` | `Array<string>` |  |

### LoyaltyTierOutput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `minPoints` | `number` |  |
| `multiplier` | `number` |  |
| `perks` | `Array<string>` |  |

### LoyaltyTransactionOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `accountId` | `string` |  |
| `points` | `number` |  |
| `transactionType` | `LoyaltyTransactionType` | Types: [`LoyaltyTransactionType`](#loyaltytransactiontype) |
| `referenceId?` | `string` |  |
| `description?` | `string` |  |
| `createdAt` | `string` |  |

### LoyaltyTransactionType

Loyalty points ledger entry kind, shared by outputs and `AdjustPointsInput.transactionType` (case-insensitive on input).

```ts
type LoyaltyTransactionType = 'earn' | 'redeem' | 'adjust' | 'expire' | 'bonus' | 'refund'
```

One of: `'earn'`, `'redeem'`, `'adjust'`, `'expire'`, `'bonus'`, `'refund'`.

### MapPurgatoryLineInput

| Field | Type | Description |
|---|---|---|
| `productId?` | `string` |  |
| `ignoreItem?` | `boolean` |  |
| `nonPhysical?` | `boolean` |  |

### MappingLookupInput

| Field | Type | Description |
|---|---|---|
| `integration` | `string` |  |
| `mappingGroup` | `string` |  |
| `fieldName` | `string` |  |
| `externalValue` | `string` |  |

### MoneyWire

Exact money on the wire (`stateset_primitives::MoneyWire`).

| Field | Type | Description |
|---|---|---|
| `amount` | `string` | Base-10 decimal amount, for example `"29.99"`. |
| `currency` | `string` | ISO 4217 code that fixes the permitted minor-unit scale, e.g. `"USD"`. |

### NcrFilterInput

Filter for `quality.listNcrs()`

| Field | Type | Description |
|---|---|---|
| `source?` | `'inspection' \| 'customer_complaint' \| 'internal_audit' \| 'supplier_issue' \| 'production_defect' \| 'shipping_damage'` | Where the non-conformance was found. |
| `severity?` | `'critical' \| 'major' \| 'minor' \| 'observation'` | Severity. |
| `status?` | `'open' \| 'under_review' \| 'pending_disposition' \| 'corrective_action' \| 'preventive_action' \| 'verification' \| 'closed' \| 'cancelled'` | NCR status. |
| `sku?` | `string` |  |
| `lotNumber?` | `string` |  |
| `assignedTo?` | `string` |  |
| `fromDate?` | `string` | RFC 3339 timestamp (inclusive lower bound on created_at) |
| `toDate?` | `string` | RFC 3339 timestamp (inclusive upper bound on created_at) |
| `limit?` | `number` | Page size (server default 500, hard cap 1000) |
| `offset?` | `number` |  |
| `afterCursor?` | `Array<string>` | Keyset cursor: `[createdAt, id]` |

### NcrOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `ncrNumber` | `string` |  |
| `source` | `NcrSource` | Types: [`NcrSource`](#ncrsource) |
| `severity` | `NcrSeverity` | Types: [`NcrSeverity`](#ncrseverity) |
| `sku` | `string` |  |
| `quantityAffected` | `number` |  |
| `status` | `NcrStatus` | Types: [`NcrStatus`](#ncrstatus) |
| `description` | `string` |  |
| `createdAt` | `string` |  |

### NcrSeverity

Non-conformance severity as rendered on `NcrOutput.severity` (Rust `Debug` form).

```ts
type NcrSeverity = 'Critical' | 'Major' | 'Minor' | 'Observation'
```

One of: `'Critical'`, `'Major'`, `'Minor'`, `'Observation'`.

### NcrSeverityInput

Non-conformance severity accepted by `CreateNcrInput.severity` (case-insensitive).

```ts
type NcrSeverityInput = 'critical' | 'major' | 'minor' | 'observation'
```

One of: `'critical'`, `'major'`, `'minor'`, `'observation'`.

### NcrSource

Non-conformance source as rendered on `NcrOutput.source` (Rust `Debug` form).

```ts
type NcrSource = 'Inspection' | 'CustomerComplaint' | 'InternalAudit' | 'SupplierIssue' | 'ProductionDefect' | 'ShippingDamage'
```

One of: `'Inspection'`, `'CustomerComplaint'`, `'InternalAudit'`, `'SupplierIssue'`, `'ProductionDefect'`, `'ShippingDamage'`.

### NcrSourceInput

Non-conformance source accepted by `CreateNcrInput.source` (case-insensitive).

```ts
type NcrSourceInput = 'inspection' | 'production' | 'production_defect' | 'customer' | 'customer_complaint' | 'supplier' | 'supplier_issue' | 'internal_audit' | 'shipping_damage'
```

One of: `'inspection'`, `'production'`, `'production_defect'`, `'customer'`, `'customer_complaint'`, `'supplier'`, `'supplier_issue'`, `'internal_audit'`, `'shipping_damage'`.

### NcrStatus

Non-conformance report status as rendered on `NcrOutput.status` (Rust `Debug` form).

```ts
type NcrStatus = 'Open' | 'UnderReview' | 'PendingDisposition' | 'CorrectiveAction' | 'PreventiveAction' | 'Verification' | 'Closed' | 'Cancelled'
```

One of: `'Open'`, `'UnderReview'`, `'PendingDisposition'`, `'CorrectiveAction'`, `'PreventiveAction'`, `'Verification'`, `'Closed'`, `'Cancelled'`.

### OpenOptions

Options for `Commerce.open`.

| Field | Type | Description |
|---|---|---|
| `maxConnections?` | `number` | Size of the connection pool. Defaults to the engine's own default. |

### OrderAddressInput

| Field | Type | Description |
|---|---|---|
| `line1` | `string` |  |
| `line2?` | `string` |  |
| `city` | `string` |  |
| `state?` | `string` |  |
| `postalCode` | `string` |  |
| `country` | `string` |  |

### OrderAddressOutput

| Field | Type | Description |
|---|---|---|
| `line1` | `string` |  |
| `line2?` | `string` |  |
| `city` | `string` |  |
| `state?` | `string` |  |
| `postalCode` | `string` |  |
| `country` | `string` |  |

### OrderFilterInput

Filter for `orders.list()`

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `status?` | `OrderStatus \| 'partiallyshipped' \| 'canceled'` | Order status (`canceled` and the unseparated `partiallyshipped` are accepted too) Types: [`OrderStatus`](#orderstatus) |
| `paymentStatus?` | `PaymentStatus \| 'partiallypaid' \| 'partiallyrefunded'` | Order-level payment status Types: [`PaymentStatus`](#paymentstatus) |
| `fulfillmentStatus?` | `FulfillmentStatus \| 'partiallyfulfilled'` | Order-level fulfillment status Types: [`FulfillmentStatus`](#fulfillmentstatus) |
| `fromDate?` | `string` | RFC 3339 timestamp (inclusive lower bound on order date) |
| `toDate?` | `string` | RFC 3339 timestamp (inclusive upper bound on order date) |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |
| `afterCursor?` | `Array<string>` | Keyset cursor: `[orderDate, id]` |

### OrderItemOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `sku` | `string` |  |
| `name` | `string` |  |
| `quantity` | `number` |  |
| `unitPrice` | `number` | **Deprecated.** Use the `unitPriceExact` twin; float money will be removed in 2.0. |
| `unitPriceExact` | `string` | Exact base-10 unit price. Prefer this field for calculations. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `total` | `number` | **Deprecated.** Use the `totalExact` twin; float money will be removed in 2.0. |
| `totalExact` | `string` | Exact base-10 line total. Prefer this field for calculations. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### OrderOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `orderNumber` | `string` |  |
| `customerId` | `string` |  |
| `status` | `OrderStatus` | Types: [`OrderStatus`](#orderstatus) |
| `totalAmount` | `number` | **Deprecated.** Use the `totalAmountExact` twin; float money will be removed in 2.0. |
| `totalAmountExact` | `string` | Exact base-10 order total. Prefer this field for calculations. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `currency` | `string` |  |
| `paymentStatus` | `PaymentStatus` | Types: [`PaymentStatus`](#paymentstatus) |
| `fulfillmentStatus` | `FulfillmentStatus` | Types: [`FulfillmentStatus`](#fulfillmentstatus) |
| `trackingNumber?` | `string` |  |
| `shippingMethod?` | `string` |  |
| `notes?` | `string` |  |
| `shippingAddress?` | `OrderAddressOutput` | Types: [`OrderAddressOutput`](#orderaddressoutput) |
| `billingAddress?` | `OrderAddressOutput` | Types: [`OrderAddressOutput`](#orderaddressoutput) |
| `items` | `Array<OrderItemOutput>` | Types: [`OrderItemOutput`](#orderitemoutput) |
| `version` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### OrderSearchResultOutput

| Field | Type | Description |
|---|---|---|
| `order` | `OrderOutput` | Types: [`OrderOutput`](#orderoutput) |
| `distance` | `number` |  |
| `score` | `number` |  |

### OrderStatus

Order lifecycle status as rendered on `OrderOutput.status` and accepted by `OrderFilterInput.status`.

```ts
type OrderStatus = 'pending' | 'confirmed' | 'processing' | 'partially_shipped' | 'shipped' | 'delivered' | 'cancelled' | 'refunded'
```

One of: `'pending'`, `'confirmed'`, `'processing'`, `'partially_shipped'`, `'shipped'`, `'delivered'`, `'cancelled'`, `'refunded'`.

### OrderStatusBreakdownOutput

| Field | Type | Description |
|---|---|---|
| `pending` | `number` |  |
| `confirmed` | `number` |  |
| `processing` | `number` |  |
| `shipped` | `number` |  |
| `delivered` | `number` |  |
| `cancelled` | `number` |  |
| `refunded` | `number` |  |

### OrderStatusUpdate

The statuses `orders.updateStatus` accepts (`partially_shipped` is derived by `ship` and cannot be set directly).

```ts
type OrderStatusUpdate = 'pending' | 'confirmed' | 'processing' | 'shipped' | 'delivered' | 'cancelled' | 'refunded'
```

One of: `'pending'`, `'confirmed'`, `'processing'`, `'shipped'`, `'delivered'`, `'cancelled'`, `'refunded'`.

### PairStationResultOutput

| Field | Type | Description |
|---|---|---|
| `station` | `PrintStationOutput` | Types: [`PrintStationOutput`](#printstationoutput) |
| `token` | `string` | One-time pairing token; shown only at pairing time |

### PauseSubscriptionInput

| Field | Type | Description |
|---|---|---|
| `reason?` | `string` |  |
| `resumeAt?` | `string` |  |

### PaymentFilterInput

Filter for `payments.list()`

| Field | Type | Description |
|---|---|---|
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `customerId?` | `string` |  |
| `status?` | `PaymentTransactionStatus \| 'canceled'` | Payment processing status (`canceled` is accepted too) Types: [`PaymentTransactionStatus`](#paymenttransactionstatus) |
| `paymentMethod?` | `PaymentMethodType` | Payment method Types: [`PaymentMethodType`](#paymentmethodtype) |
| `currency?` | `string` | ISO 4217 currency code |
| `minAmount?` | `string` | Exact decimal string (inclusive lower bound on amount) |
| `maxAmount?` | `string` | Exact decimal string (inclusive upper bound on amount) |
| `fromDate?` | `string` | RFC 3339 timestamp (inclusive lower bound on created_at) |
| `toDate?` | `string` | RFC 3339 timestamp (inclusive upper bound on created_at) |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |

### PaymentMethodType

Payment method accepted by `CreatePaymentInput.paymentMethod` and `PaymentFilterInput.paymentMethod`, aliases included (`ach`, `cod`, `usdc`, ...).

```ts
type PaymentMethodType = 'credit_card' | 'debit_card' | 'bank_transfer' | 'ach' | 'paypal' | 'apple_pay' | 'google_pay' | 'crypto' | 'cryptocurrency' | 'stablecoin' | 'usdc' | 'usdt' | 'ssusd' | 'store_credit' | 'gift_card' | 'cash_on_delivery' | 'cod' | 'invoice' | 'other'
```

One of: `'credit_card'`, `'debit_card'`, `'bank_transfer'`, `'ach'`, `'paypal'`, `'apple_pay'`, `'google_pay'`, `'crypto'`, `'cryptocurrency'`, `'stablecoin'`, `'usdc'`, `'usdt'`, `'ssusd'`, `'store_credit'`, `'gift_card'`, `'cash_on_delivery'`, `'cod'`, `'invoice'`, `'other'`.

### PaymentObligationDashboardOutput

| Field | Type | Description |
|---|---|---|
| `openCount` | `string` |  |
| `totalOutstanding` | `string` | Exact decimal string |
| `overdueCount` | `string` |  |
| `overdueAmount` | `string` | Exact decimal string |

### PaymentObligationFilterInput

| Field | Type | Description |
|---|---|---|
| `supplierId?` | `string` |  |
| `status?` | `PaymentObligationStatus` | Snake-case status Types: [`PaymentObligationStatus`](#paymentobligationstatus) |
| `dueBefore?` | `string` | Date string (YYYY-MM-DD) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### PaymentObligationOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `number` | `string` |  |
| `supplierId` | `string` |  |
| `purchaseOrderId?` | `string` |  |
| `amount` | `string` | Exact decimal string |
| `amountPaid` | `string` | Exact decimal string |
| `outstanding` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `dueDate` | `string` | Date string (YYYY-MM-DD) |
| `status` | `PaymentObligationStatus` | Snake-case status Types: [`PaymentObligationStatus`](#paymentobligationstatus) |
| `linkedBillIds` | `Array<string>` |  |
| `notes?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PaymentObligationStatus

Payment obligation lifecycle status (`PaymentObligationStatus`).

```ts
type PaymentObligationStatus = 'pending' | 'scheduled' | 'partially_paid' | 'paid' | 'cancelled'
```

One of: `'pending'`, `'scheduled'`, `'partially_paid'`, `'paid'`, `'cancelled'`.

### PaymentOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `paymentNumber` | `string` |  |
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `customerId?` | `string` |  |
| `idempotencyKey?` | `string` |  |
| `amount` | `number` | **Deprecated.** Use the `amountExact` twin; float money will be removed in 2.0. |
| `amountExact` | `string` | Exact base-10 amount. Prefer this field for all calculations. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `currency` | `string` |  |
| `status` | `PaymentTransactionStatus` | Types: [`PaymentTransactionStatus`](#paymenttransactionstatus) |
| `version` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PaymentStatus

Order-level payment status (`OrderOutput.paymentStatus`, `OrderFilterInput.paymentStatus`).

```ts
type PaymentStatus = 'pending' | 'authorized' | 'paid' | 'partially_paid' | 'refunded' | 'partially_refunded' | 'failed'
```

One of: `'pending'`, `'authorized'`, `'paid'`, `'partially_paid'`, `'refunded'`, `'partially_refunded'`, `'failed'`.

### PaymentTransactionStatus

Payment processing status (`PaymentOutput.status`, `PaymentFilterInput.status`).

```ts
type PaymentTransactionStatus = 'pending' | 'processing' | 'requires_action' | 'completed' | 'failed' | 'cancelled' | 'refunded' | 'partially_refunded' | 'disputed'
```

One of: `'pending'`, `'processing'`, `'requires_action'`, `'completed'`, `'failed'`, `'cancelled'`, `'refunded'`, `'partially_refunded'`, `'disputed'`.

### PerformanceObligationOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `contractId` | `string` |  |
| `description` | `string` |  |
| `standaloneSellingPrice?` | `string` | Exact decimal string |
| `allocatedAmount` | `string` | Exact decimal string |
| `recognitionMethod` | `RecognitionMethodOutput` | point_in_time, ratable_over_time, milestone Types: [`RecognitionMethodOutput`](#recognitionmethodoutput) |
| `recognitionStart?` | `string` | ISO date (YYYY-MM-DD); set for ratable_over_time |
| `recognitionEnd?` | `string` | ISO date (YYYY-MM-DD); set for ratable_over_time |
| `recognizedAmount` | `string` | Exact decimal string |
| `deferredAmount` | `string` | Exact decimal string: allocated_amount - recognized_amount |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PickTaskFilterInput

Optional filters for `Fulfillment.listPicks`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `warehouseId?` | `number` |  |
| `waveId?` | `string` |  |
| `orderId?` | `string` |  |
| `status?` | `PickTaskStatusInput` | The rendered form (`InProgress`) or the engine's snake_case (`in_progress`). Types: [`PickTaskStatusInput`](#picktaskstatusinput) |
| `assignedTo?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### PickTaskOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `waveId?` | `string` |  |
| `orderId` | `string` |  |
| `sku` | `string` |  |
| `quantityRequested` | `number` |  |
| `quantityPicked` | `number` |  |
| `status` | `PickTaskStatus` | Types: [`PickTaskStatus`](#picktaskstatus) |
| `sourceLocationId` | `number` |  |

### PickTaskStatus

Pick task status as rendered on `PickTaskOutput.status` (Rust `Debug` form).

```ts
type PickTaskStatus = 'Pending' | 'Assigned' | 'InProgress' | 'Completed' | 'Short' | 'Cancelled'
```

One of: `'Pending'`, `'Assigned'`, `'InProgress'`, `'Completed'`, `'Short'`, `'Cancelled'`.

### PickTaskStatusInput

Pick task status accepted by `PickTaskFilterInput.status`: the rendered form or the engine's snake_case (strict).

```ts
type PickTaskStatusInput = PickTaskStatus | 'pending' | 'assigned' | 'in_progress' | 'completed' | 'short' | 'cancelled' | 'canceled'
```

Types: [`PickTaskStatus`](#picktaskstatus)

### PortableDomainsOutput

Domains that structured export/import can cover.

| Field | Type | Description |
|---|---|---|
| `exportable` | `Array<string>` |  |
| `importable` | `Array<string>` |  |

### PrepaymentApplicationOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `prepaymentId` | `string` |  |
| `targetType` | `PrepaymentTargetType` | bill or payment_obligation Types: [`PrepaymentTargetType`](#prepaymenttargettype) |
| `targetId` | `string` |  |
| `amount` | `string` | Exact decimal string |
| `reversed` | `boolean` |  |
| `createdAt` | `string` |  |

### PrepaymentFilterInput

| Field | Type | Description |
|---|---|---|
| `supplierId?` | `string` |  |
| `status?` | `PrepaymentStatus` | open, applied, refunded, cancelled Types: [`PrepaymentStatus`](#prepaymentstatus) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### PrepaymentOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `number` | `string` |  |
| `supplierId` | `string` |  |
| `amount` | `string` | Exact decimal string |
| `remaining` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `status` | `PrepaymentStatus` | open, applied, refunded, cancelled Types: [`PrepaymentStatus`](#prepaymentstatus) |
| `method?` | `string` |  |
| `reference?` | `string` |  |
| `memo?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PrepaymentStatus

Prepayment lifecycle status (`PrepaymentStatus`).

```ts
type PrepaymentStatus = 'open' | 'applied' | 'refunded' | 'cancelled'
```

One of: `'open'`, `'applied'`, `'refunded'`, `'cancelled'`.

### PrepaymentTargetType

What a prepayment application is applied against (`PrepaymentTargetType`).

```ts
type PrepaymentTargetType = 'bill' | 'payment_obligation'
```

One of: `'bill'`, `'payment_obligation'`.

### PriceAdjustmentType

Catalog-wide adjustment a price level applies by default (`PriceAdjustmentType`).

```ts
type PriceAdjustmentType = 'none' | 'percentage_discount' | 'percentage_markup'
```

One of: `'none'`, `'percentage_discount'`, `'percentage_markup'`.

### PriceLevelEntryOutput

| Field | Type | Description |
|---|---|---|
| `priceLevelId` | `string` |  |
| `productId` | `string` |  |
| `price` | `string` | Exact decimal string |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PriceLevelFilterInput

| Field | Type | Description |
|---|---|---|
| `isActive?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### PriceLevelOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `code` | `string` |  |
| `description?` | `string` |  |
| `adjustmentType` | `PriceAdjustmentType` | none, percentage_discount, percentage_markup Types: [`PriceAdjustmentType`](#priceadjustmenttype) |
| `adjustmentValue` | `string` | Percentage as exact decimal string |
| `currency` | `string` |  |
| `isActive` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PriceScheduleEntryOutput

| Field | Type | Description |
|---|---|---|
| `priceScheduleId` | `string` |  |
| `productId` | `string` |  |
| `price` | `string` | Exact decimal string |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PriceScheduleFilterInput

| Field | Type | Description |
|---|---|---|
| `isActive?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### PriceScheduleOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `code?` | `string` |  |
| `currency` | `string` |  |
| `startsAt?` | `string` | RFC 3339 timestamp |
| `endsAt?` | `string` | RFC 3339 timestamp |
| `isActive` | `boolean` |  |
| `priority` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PrintJobFilterInput

| Field | Type | Description |
|---|---|---|
| `status?` | `PrintJobStatus` | queued, picked_up, printed, failed Types: [`PrintJobStatus`](#printjobstatus) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### PrintJobOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `stationId` | `string` |  |
| `printerName?` | `string` |  |
| `payloadKind` | `PrintPayloadKind` | zpl or pdf Types: [`PrintPayloadKind`](#printpayloadkind) |
| `payload` | `string` |  |
| `status` | `PrintJobStatus` | queued, picked_up, printed, failed Types: [`PrintJobStatus`](#printjobstatus) |
| `createdAt` | `string` |  |
| `pickedUpAt?` | `string` |  |

### PrintJobStatus

Print job lifecycle status (`PrintJobStatus`).

```ts
type PrintJobStatus = 'queued' | 'picked_up' | 'printed' | 'failed'
```

One of: `'queued'`, `'picked_up'`, `'printed'`, `'failed'`.

### PrintPayloadKind

Print job payload encoding (`PrintPayloadKind`).

```ts
type PrintPayloadKind = 'zpl' | 'pdf'
```

One of: `'zpl'`, `'pdf'`.

### PrintStationFilterInput

Filter for `listStations()`; omit for every station.

| Field | Type | Description |
|---|---|---|
| `revoked?` | `boolean` | Only revoked (`true`) or only live (`false`) stations |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### PrintStationOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `printers` | `Array<string>` |  |
| `revoked` | `boolean` |  |
| `lastSeenAt?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ProductFilterInput

Filter for `products.list()`

| Field | Type | Description |
|---|---|---|
| `status?` | `ProductStatus` | Catalogue status Types: [`ProductStatus`](#productstatus) |
| `search?` | `string` | Free-text search over name and description |
| `category?` | `string` | Matches the product's `category` attribute |
| `minPrice?` | `string` | Exact decimal string (inclusive lower bound on variant price) |
| `maxPrice?` | `string` | Exact decimal string (inclusive upper bound on variant price) |
| `inStock?` | `boolean` |  |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |
| `afterCursor?` | `Array<string>` | Keyset cursor: `[name, id]` |

### ProductOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `slug` | `string` |  |
| `description` | `string` |  |
| `category?` | `string` |  |
| `status` | `ProductStatus` | Types: [`ProductStatus`](#productstatus) |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ProductPerformanceOutput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `sku` | `string` |  |
| `name` | `string` |  |
| `unitsSold` | `number` |  |
| `revenue` | `number` | **Deprecated.** Use the `revenueExact` twin; float money will be removed in 2.0. |
| `revenueExact` | `string` | Exact base-10 revenue, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `previousUnitsSold` | `number` |  |
| `previousRevenue` | `number` | **Deprecated.** Use the `previousRevenueExact` twin; float money will be removed in 2.0. |
| `previousRevenueExact` | `string` | Exact base-10 previous revenue, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `unitsGrowthPercent` | `number` |  |
| `revenueGrowthPercent` | `number` |  |

### ProductSearchResultOutput

| Field | Type | Description |
|---|---|---|
| `product` | `ProductOutput` | Types: [`ProductOutput`](#productoutput) |
| `distance` | `number` |  |
| `score` | `number` |  |

### ProductStatus

Catalogue status (`ProductOutput.status`, `UpdateProductInput.status`, `ProductFilterInput.status`).

```ts
type ProductStatus = 'draft' | 'active' | 'archived'
```

One of: `'draft'`, `'active'`, `'archived'`.

### ProductTaxCategory

Product tax category, shared by outputs and inputs (case-insensitive on input).

```ts
type ProductTaxCategory = 'standard' | 'reduced' | 'super_reduced' | 'zero_rated' | 'exempt' | 'digital' | 'clothing' | 'food' | 'prepared_food' | 'medical' | 'educational' | 'luxury'
```

One of: `'standard'`, `'reduced'`, `'super_reduced'`, `'zero_rated'`, `'exempt'`, `'digital'`, `'clothing'`, `'food'`, `'prepared_food'`, `'medical'`, `'educational'`, `'luxury'`.

### ProductVariantOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `productId` | `string` |  |
| `sku` | `string` |  |
| `name` | `string` |  |
| `price` | `number` | **Deprecated.** Use the `priceExact` twin; float money will be removed in 2.0. |
| `priceExact` | `string` | Exact base-10 price. Prefer this field for calculations. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `compareAtPrice?` | `number` | **Deprecated.** Use the `compareAtPriceExact` twin; float money will be removed in 2.0. |
| `compareAtPriceExact?` | `string` | Exact base-10 comparison price. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `isDefault` | `boolean` |  |

### ProductionBatchFilterInput

| Field | Type | Description |
|---|---|---|
| `status?` | `ProductionBatchStatus` | planned, in_progress, completed, cancelled Types: [`ProductionBatchStatus`](#productionbatchstatus) |
| `vendorId?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### ProductionBatchOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `status` | `ProductionBatchStatus` | planned, in_progress, completed, cancelled Types: [`ProductionBatchStatus`](#productionbatchstatus) |
| `vendorId?` | `string` |  |
| `workOrderIds` | `Array<string>` |  |
| `notes?` | `string` |  |
| `scheduledStart?` | `string` | RFC 3339 timestamp |
| `scheduledEnd?` | `string` | RFC 3339 timestamp |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ProductionBatchStatus

Production batch lifecycle status (`ProductionBatchStatus`).

```ts
type ProductionBatchStatus = 'planned' | 'in_progress' | 'completed' | 'cancelled'
```

One of: `'planned'`, `'in_progress'`, `'completed'`, `'cancelled'`.

### PromotionFilterInput

Filter for listing promotions

| Field | Type | Description |
|---|---|---|
| `status?` | `PromotionStatus` | Filter by status Types: [`PromotionStatus`](#promotionstatus) |
| `promotionType?` | `PromotionTypeInput` | Filter by promotion type Types: [`PromotionTypeInput`](#promotiontypeinput) |
| `trigger?` | `PromotionTriggerInput` | Filter by trigger Types: [`PromotionTriggerInput`](#promotiontriggerinput) |
| `isActive?` | `boolean` | Filter by active status |
| `search?` | `string` | Search term |
| `limit?` | `number` | Max results |
| `offset?` | `number` | Offset for pagination |

### PromotionLineItemInput

Line item input for promotion calculation

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `productId?` | `string` |  |
| `variantId?` | `string` |  |
| `sku?` | `string` |  |
| `categoryIds?` | `Array<string>` |  |
| `quantity` | `number` |  |
| `unitPrice` | `number` |  |
| `lineTotal` | `number` |  |

### PromotionOutput

Promotion output

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `code` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `internalNotes?` | `string` |  |
| `promotionType` | `PromotionType` | Types: [`PromotionType`](#promotiontype) |
| `trigger` | `PromotionTrigger` | Types: [`PromotionTrigger`](#promotiontrigger) |
| `target` | `PromotionTarget` | Types: [`PromotionTarget`](#promotiontarget) |
| `stacking` | `PromotionStacking` | Types: [`PromotionStacking`](#promotionstacking) |
| `status` | `PromotionStatus` | Types: [`PromotionStatus`](#promotionstatus) |
| `percentageOff?` | `number` |  |
| `fixedAmountOff?` | `number` | **Deprecated.** Use the `fixedAmountOffExact` twin; float money will be removed in 2.0. |
| `fixedAmountOffExact?` | `string` | Exact base-10 fixed amount off, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `maxDiscountAmount?` | `number` | **Deprecated.** Use the `maxDiscountAmountExact` twin; float money will be removed in 2.0. |
| `maxDiscountAmountExact?` | `string` | Exact base-10 max discount amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `buyQuantity?` | `number` |  |
| `getQuantity?` | `number` |  |
| `getDiscountPercent?` | `number` |  |
| `startsAt` | `string` |  |
| `endsAt?` | `string` |  |
| `totalUsageLimit?` | `number` |  |
| `perCustomerLimit?` | `number` |  |
| `usageCount` | `number` |  |
| `currency` | `string` |  |
| `priority` | `number` |  |
| `metadata?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PromotionStacking

Stacking behaviour as rendered on `PromotionOutput.stacking` (lower-cased Rust `Debug` form).

```ts
type PromotionStacking = 'stackable' | 'exclusive' | 'selectivestack'
```

One of: `'stackable'`, `'exclusive'`, `'selectivestack'`.

### PromotionStackingInput

Stacking behaviour accepted on input (case-insensitive).

```ts
type PromotionStackingInput = 'stackable' | 'exclusive' | 'selective_stack' | 'selectivestack'
```

One of: `'stackable'`, `'exclusive'`, `'selective_stack'`, `'selectivestack'`.

### PromotionStatus

Promotion status, shared by `PromotionOutput.status` and the update/filter inputs (case-insensitive on input).

```ts
type PromotionStatus = 'draft' | 'scheduled' | 'active' | 'paused' | 'expired' | 'exhausted' | 'archived'
```

One of: `'draft'`, `'scheduled'`, `'active'`, `'paused'`, `'expired'`, `'exhausted'`, `'archived'`.

### PromotionTarget

Promotion target as rendered on `PromotionOutput.target` (lower-cased Rust `Debug` form).

```ts
type PromotionTarget = 'order' | 'product' | 'category' | 'shipping' | 'lineitem'
```

One of: `'order'`, `'product'`, `'category'`, `'shipping'`, `'lineitem'`.

### PromotionTargetInput

Promotion target accepted on input (case-insensitive).

```ts
type PromotionTargetInput = 'order' | 'product' | 'category' | 'shipping' | 'line_item' | 'lineitem'
```

One of: `'order'`, `'product'`, `'category'`, `'shipping'`, `'line_item'`, `'lineitem'`.

### PromotionTrigger

Promotion trigger as rendered on `PromotionOutput.trigger` (lower-cased Rust `Debug` form).

```ts
type PromotionTrigger = 'automatic' | 'couponcode' | 'both'
```

One of: `'automatic'`, `'couponcode'`, `'both'`.

### PromotionTriggerInput

Promotion trigger accepted on input (case-insensitive).

```ts
type PromotionTriggerInput = 'automatic' | 'auto' | 'coupon_code' | 'couponcode' | 'coupon' | 'both'
```

One of: `'automatic'`, `'auto'`, `'coupon_code'`, `'couponcode'`, `'coupon'`, `'both'`.

### PromotionType

Promotion type as rendered on `PromotionOutput.promotionType` and `AppliedPromotionOutput.discountType` (lower-cased Rust `Debug` form).

```ts
type PromotionType = 'percentageoff' | 'fixedamountoff' | 'buyxgety' | 'freeshipping' | 'tiereddiscount' | 'bundlediscount' | 'firstorderdiscount' | 'giftwithpurchase'
```

One of: `'percentageoff'`, `'fixedamountoff'`, `'buyxgety'`, `'freeshipping'`, `'tiereddiscount'`, `'bundlediscount'`, `'firstorderdiscount'`, `'giftwithpurchase'`.

### PromotionTypeInput

Promotion type accepted on input (case-insensitive).

```ts
type PromotionTypeInput = 'percentage_off' | 'percentageoff' | 'fixed_amount_off' | 'fixedamountoff' | 'buy_x_get_y' | 'buyxgety' | 'bogo' | 'free_shipping' | 'freeshipping' | 'tiered_discount' | 'tiereddiscount' | 'bundle' | 'bundle_discount' | 'bundlediscount'
```

One of: `'percentage_off'`, `'percentageoff'`, `'fixed_amount_off'`, `'fixedamountoff'`, `'buy_x_get_y'`, `'buyxgety'`, `'bogo'`, `'free_shipping'`, `'freeshipping'`, `'tiered_discount'`, `'tiereddiscount'`, `'bundle'`, `'bundle_discount'`, `'bundlediscount'`.

### PromotionUsageOutput

Promotion usage record output

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `promotionId` | `string` |  |
| `couponId?` | `string` |  |
| `customerId?` | `string` |  |
| `orderId?` | `string` |  |
| `cartId?` | `string` |  |
| `discountAmount` | `number` | **Deprecated.** Use the `discountAmountExact` twin; float money will be removed in 2.0. |
| `discountAmountExact` | `string` | Exact base-10 discount amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `currency` | `string` |  |
| `usedAt` | `string` |  |

### PurchaseOrderFilterInput

Filter for `purchaseOrders.list()`

| Field | Type | Description |
|---|---|---|
| `supplierId?` | `string` |  |
| `status?` | `PurchaseOrderStatus \| 'canceled'` | Purchase order status (`canceled` is accepted as an alias of `cancelled`). Types: [`PurchaseOrderStatus`](#purchaseorderstatus) |
| `fromDate?` | `string` | RFC 3339 timestamp (inclusive lower bound on order date) |
| `toDate?` | `string` | RFC 3339 timestamp (inclusive upper bound on order date) |
| `minTotal?` | `string` | Exact decimal string |
| `maxTotal?` | `string` | Exact decimal string |
| `limit?` | `number` | Page size (server default 500, hard cap 1000) |
| `offset?` | `number` |  |
| `afterCursor?` | `Array<string>` | Keyset cursor: `[orderDate, id]` |

### PurchaseOrderOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `poNumber` | `string` |  |
| `supplierId` | `string` |  |
| `status` | `PurchaseOrderStatus` | Types: [`PurchaseOrderStatus`](#purchaseorderstatus) |
| `subtotal` | `number` | **Deprecated.** Use the `subtotalExact` twin; float money will be removed in 2.0. |
| `subtotalExact` | `string` | Exact base-10 subtotal, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `total` | `number` | **Deprecated.** Use the `totalExact` twin; float money will be removed in 2.0. |
| `totalExact` | `string` | Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### PurchaseOrderStatus

Purchase order status (`PurchaseOrderOutput.status`, `PurchaseOrderFilterInput.status`).

```ts
type PurchaseOrderStatus = 'draft' | 'pending_approval' | 'approved' | 'sent' | 'acknowledged' | 'partially_received' | 'received' | 'completed' | 'cancelled' | 'on_hold'
```

One of: `'draft'`, `'pending_approval'`, `'approved'`, `'sent'`, `'acknowledged'`, `'partially_received'`, `'received'`, `'completed'`, `'cancelled'`, `'on_hold'`.

### PurgatoryFilterInput

| Field | Type | Description |
|---|---|---|
| `channelId?` | `string` |  |
| `isPosted?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### PurgatoryLineItemOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `purgatoryOrderId` | `string` |  |
| `externalSku` | `string` |  |
| `productId?` | `string` |  |
| `quantity` | `string` | Exact decimal string |
| `ignoreItem` | `boolean` |  |
| `nonPhysical` | `boolean` |  |
| `isResolved` | `boolean` |  |

### PurgatoryOrderOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `channelId?` | `string` |  |
| `externalOrderId` | `string` |  |
| `externalStatus?` | `string` |  |
| `isPosted` | `boolean` |  |
| `holdReason?` | `string` |  |
| `metadata` | `string` | JSON string |
| `items` | `Array<PurgatoryLineItemOutput>` | Types: [`PurgatoryLineItemOutput`](#purgatorylineitemoutput) |
| `isReadyToPost` | `boolean` |  |
| `unresolvedCount` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### QualityHoldFilterInput

Optional filters for `Quality.listHolds`. No argument lists every hold.

| Field | Type | Description |
|---|---|---|
| `sku?` | `string` |  |
| `lotNumber?` | `string` |  |
| `holdType?` | `QualityHoldTypeFilter` | The rendered form (`RegulatoryHold`) or the engine's snake_case (`regulatory_hold`). Types: [`QualityHoldTypeFilter`](#qualityholdtypefilter) |
| `locationId?` | `number` |  |
| `activeOnly?` | `boolean` | Only holds that have not been released. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### QualityHoldOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `sku` | `string` |  |
| `lotNumber?` | `string` |  |
| `quantityHeld` | `number` |  |
| `reason` | `string` |  |
| `holdType` | `QualityHoldType` | Types: [`QualityHoldType`](#qualityholdtype) |
| `status` | `QualityHoldStatus` | Types: [`QualityHoldStatus`](#qualityholdstatus) |
| `placedAt` | `string` |  |

### QualityHoldStatus

Whether a quality hold is still in force, as rendered on `QualityHoldOutput.status`.

```ts
type QualityHoldStatus = 'held' | 'released'
```

One of: `'held'`, `'released'`.

### QualityHoldType

Quality hold type as rendered on `QualityHoldOutput.holdType` (Rust `Debug` form).

```ts
type QualityHoldType = 'QualityInspection' | 'CustomerReturn' | 'Recall' | 'Damaged' | 'Expired' | 'Quarantine' | 'RegulatoryHold' | 'InvestigationHold'
```

One of: `'QualityInspection'`, `'CustomerReturn'`, `'Recall'`, `'Damaged'`, `'Expired'`, `'Quarantine'`, `'RegulatoryHold'`, `'InvestigationHold'`.

### QualityHoldTypeFilter

Quality hold type accepted by `QualityHoldFilterInput.holdType`: the rendered form or the engine's snake_case (strict).

```ts
type QualityHoldTypeFilter = QualityHoldType | 'quality_inspection' | 'customer_return' | 'recall' | 'damaged' | 'expired' | 'quarantine' | 'regulatory_hold' | 'investigation_hold'
```

Types: [`QualityHoldType`](#qualityholdtype)

### QualityHoldTypeInput

Quality hold type accepted by `CreateQualityHoldInput.holdType` (case-insensitive).

```ts
type QualityHoldTypeInput = 'quality_inspection' | 'qualityinspection' | 'damage' | 'damaged' | 'regulatory' | 'regulatory_hold' | 'customer_return' | 'customerreturn' | 'recall' | 'expired' | 'quarantine' | 'investigation' | 'investigation_hold'
```

One of: `'quality_inspection'`, `'qualityinspection'`, `'damage'`, `'damaged'`, `'regulatory'`, `'regulatory_hold'`, `'customer_return'`, `'customerreturn'`, `'recall'`, `'expired'`, `'quarantine'`, `'investigation'`, `'investigation_hold'`.

### ReceiptFilterInput

Optional filters for `Receiving.listReceipts`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `warehouseId?` | `number` |  |
| `receiptType?` | `ReceiptTypeFilter` | The rendered form (`PurchaseOrder`) or the engine's snake_case (`purchase_order`). Types: [`ReceiptTypeFilter`](#receipttypefilter) |
| `status?` | `ReceiptStatusInput` | The rendered form (`PuttingAway`) or the engine's snake_case (`putting_away`). Types: [`ReceiptStatusInput`](#receiptstatusinput) |
| `supplierId?` | `string` |  |
| `referenceId?` | `string` |  |
| `fromDate?` | `string` | RFC 3339 timestamp. |
| `toDate?` | `string` | RFC 3339 timestamp. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### ReceiptOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `receiptNumber` | `string` |  |
| `receiptType` | `ReceiptType` | Types: [`ReceiptType`](#receipttype) |
| `warehouseId` | `number` |  |
| `status` | `ReceiptStatus` | Types: [`ReceiptStatus`](#receiptstatus) |
| `carrier?` | `string` |  |
| `trackingNumber?` | `string` |  |
| `createdAt` | `string` |  |

### ReceiptStatus

Receipt status as rendered on `ReceiptOutput.status` (Rust `Debug` form).

```ts
type ReceiptStatus = 'Expected' | 'InProgress' | 'Received' | 'Inspecting' | 'PuttingAway' | 'Completed' | 'Cancelled'
```

One of: `'Expected'`, `'InProgress'`, `'Received'`, `'Inspecting'`, `'PuttingAway'`, `'Completed'`, `'Cancelled'`.

### ReceiptStatusInput

Receipt status accepted by `ReceiptFilterInput.status`: the rendered form or the engine's snake_case (strict).

```ts
type ReceiptStatusInput = ReceiptStatus | 'expected' | 'in_progress' | 'received' | 'inspecting' | 'putting_away' | 'completed' | 'cancelled' | 'canceled'
```

Types: [`ReceiptStatus`](#receiptstatus)

### ReceiptType

Receipt type as rendered on `ReceiptOutput.receiptType` (Rust `Debug` form).

```ts
type ReceiptType = 'PurchaseOrder' | 'Transfer' | 'Return' | 'Adjustment' | 'Production' | 'Other'
```

One of: `'PurchaseOrder'`, `'Transfer'`, `'Return'`, `'Adjustment'`, `'Production'`, `'Other'`.

### ReceiptTypeFilter

Receipt type accepted by `ReceiptFilterInput.receiptType`: the rendered form or the engine's snake_case (strict).

```ts
type ReceiptTypeFilter = ReceiptType | 'purchase_order' | 'purchaseorder' | 'po' | 'transfer' | 'return' | 'returns' | 'adjustment' | 'production' | 'other'
```

Types: [`ReceiptType`](#receipttype)

### ReceiptTypeInput

Receipt type accepted by `CreateReceiptInput.receiptType` (case-insensitive).

```ts
type ReceiptTypeInput = 'purchase_order' | 'purchaseorder' | 'po' | 'return' | 'customer_return' | 'transfer' | 'adjustment'
```

One of: `'purchase_order'`, `'purchaseorder'`, `'po'`, `'return'`, `'customer_return'`, `'transfer'`, `'adjustment'`.

### RecognitionMethod

Revenue recognition method accepted on input (exact, lowercase). `ratable_over_time` also needs `recognitionStart`/`recognitionEnd`.

```ts
type RecognitionMethod = 'point_in_time' | 'ratable_over_time' | 'milestone'
```

One of: `'point_in_time'`, `'ratable_over_time'`, `'milestone'`.

### RecognitionMethodOutput

Recognition method as rendered on a record; `unknown` only for a method this binding predates.

```ts
type RecognitionMethodOutput = RecognitionMethod | 'unknown'
```

Types: [`RecognitionMethod`](#recognitionmethod)

### RecordActivityInput

| Field | Type | Description |
|---|---|---|
| `subjectType` | `string` | Subject record type (e.g. "sales_order") |
| `subjectId` | `string` |  |
| `action` | `string` | Machine action key (e.g. "status_changed") |
| `summary` | `string` |  |
| `actorKind?` | `ActorKind` | user, system, integration, agent Types: [`ActorKind`](#actorkind) |
| `actor?` | `string` |  |
| `metadata?` | `string` | Metadata as JSON |

### RecordCycleCountLineInput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `lotId?` | `string` |  |
| `countedQuantity` | `string` | Exact decimal string |

### RecordPaymentInput

| Field | Type | Description |
|---|---|---|
| `amount?` | `number` | Float amount. Optional: send `amount_exact` instead for exact money. |
| `amountExact?` | `string` | Exact base-10 amount. Takes precedence over `amount` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `paymentMethod?` | `string` |  |
| `reference?` | `string` |  |

### RefundOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `refundNumber` | `string` |  |
| `paymentId` | `string` |  |
| `amount` | `number` | **Deprecated.** Use the `amountExact` twin; float money will be removed in 2.0. |
| `amountExact` | `string` | Exact base-10 amount. Prefer this field for all calculations. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `status` | `RefundStatus` | Types: [`RefundStatus`](#refundstatus) |
| `reason?` | `string` |  |
| `createdAt` | `string` |  |
| `idempotencyKey?` | `string` |  |

### RefundStatus

Refund processing status (`RefundOutput.status`).

```ts
type RefundStatus = 'pending' | 'processing' | 'completed' | 'failed' | 'cancelled'
```

One of: `'pending'`, `'processing'`, `'completed'`, `'failed'`, `'cancelled'`.

### ReservationOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `itemId` | `number` |  |
| `quantity` | `string` |  |
| `status` | `ReservationStatus` | Types: [`ReservationStatus`](#reservationstatus) |

### ReservationStatus

Inventory reservation status (`ReservationOutput.status`).

```ts
type ReservationStatus = 'pending' | 'confirmed' | 'allocated' | 'cancelled' | 'released' | 'expired' | 'fulfilled'
```

One of: `'pending'`, `'confirmed'`, `'allocated'`, `'cancelled'`, `'released'`, `'expired'`, `'fulfilled'`.

### RestoreOptionsInput

Options controlling a restore.

| Field | Type | Description |
|---|---|---|
| `overwrite?` | `boolean` |  |
| `skipChecksum?` | `boolean` |  |
| `allowNewerSchema?` | `boolean` |  |

### RestoreReportOutput

Result of a database restore.

| Field | Type | Description |
|---|---|---|
| `targetPath` | `string` |  |
| `schemaVersion` | `string` |  |
| `sizeBytes` | `number` |  |
| `checksumVerified` | `boolean` |  |
| `replacedExisting` | `boolean` |  |

### ReturnFilterInput

Filter for `returns.list()`

| Field | Type | Description |
|---|---|---|
| `orderId?` | `string` |  |
| `customerId?` | `string` |  |
| `status?` | `ReturnStatus \| 'intransit' \| 'canceled'` | Return status (`canceled` and `intransit` are accepted too) Types: [`ReturnStatus`](#returnstatus) |
| `reason?` | `ReturnReason \| 'wrongitem' \| 'notasdescribed' \| 'changedmind' \| 'betterpricefound' \| 'nolongerneeded'` | Return reason (the unseparated spellings such as `wrongitem` are accepted too) Types: [`ReturnReason`](#returnreason) |
| `fromDate?` | `string` | RFC 3339 timestamp (inclusive lower bound on created_at) |
| `toDate?` | `string` | RFC 3339 timestamp (inclusive upper bound on created_at) |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |
| `afterCursor?` | `Array<string>` | Keyset cursor: `[createdAt, id]` |

### ReturnMetricsOutput

| Field | Type | Description |
|---|---|---|
| `totalReturns` | `number` |  |
| `returnRatePercent` | `number` |  |
| `totalRefunded` | `number` | **Deprecated.** Use the `totalRefundedExact` twin; float money will be removed in 2.0. |
| `totalRefundedExact` | `string` | Exact base-10 total refunded, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### ReturnOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `orderId` | `string` |  |
| `status` | `ReturnStatus` | Types: [`ReturnStatus`](#returnstatus) |
| `reason` | `ReturnReason` | Types: [`ReturnReason`](#returnreason) |
| `version` | `number` |  |
| `createdAt` | `string` |  |
| `idempotencyKey?` | `string` |  |

### ReturnReason

Why a return was requested (`ReturnOutput.reason`, `CreateReturnInput.reason`, `ReturnFilterInput.reason`); an unrecognised reason is stored as `other`.

```ts
type ReturnReason = 'defective' | 'wrong_item' | 'not_as_described' | 'changed_mind' | 'better_price_found' | 'no_longer_needed' | 'damaged' | 'other'
```

One of: `'defective'`, `'wrong_item'`, `'not_as_described'`, `'changed_mind'`, `'better_price_found'`, `'no_longer_needed'`, `'damaged'`, `'other'`.

### ReturnStatus

Return lifecycle status (`ReturnOutput.status`, `ReturnFilterInput.status`).

```ts
type ReturnStatus = 'requested' | 'approved' | 'rejected' | 'in_transit' | 'received' | 'inspecting' | 'completed' | 'cancelled'
```

One of: `'requested'`, `'approved'`, `'rejected'`, `'in_transit'`, `'received'`, `'inspecting'`, `'completed'`, `'cancelled'`.

### RevaluationLineOutput

| Field | Type | Description |
|---|---|---|
| `accountId` | `string` |  |
| `accountNumber` | `string` |  |
| `accountName` | `string` |  |
| `currency` | `string` |  |
| `normalBalance` | `GlBalanceSide` | Side that increases this account: debit or credit Types: [`GlBalanceSide`](#glbalanceside) |
| `foreignBalance` | `string` | Exact decimal string |
| `carryingValue` | `string` | Exact decimal string |
| `rate` | `string` | Exact decimal string |
| `revaluedValue` | `string` | Exact decimal string |
| `adjustment` | `string` | Exact decimal string |
| `unrealizedGainLoss` | `string` | Exact decimal string |

### RevaluationOutput

| Field | Type | Description |
|---|---|---|
| `asOfDate` | `string` | ISO date (YYYY-MM-DD) |
| `baseCurrency` | `string` |  |
| `totalUnrealizedGainLoss` | `string` | Exact decimal string |
| `lines` | `Array<RevaluationLineOutput>` | Types: [`RevaluationLineOutput`](#revaluationlineoutput) |
| `journalEntry?` | `JournalEntryOutput` | Balanced adjusting entry; None when no adjustment was required. Types: [`JournalEntryOutput`](#journalentryoutput) |

### RevenueByPeriodOutput

| Field | Type | Description |
|---|---|---|
| `period` | `string` |  |
| `revenue` | `number` | **Deprecated.** Use the `revenueExact` twin; float money will be removed in 2.0. |
| `revenueExact` | `string` | Exact base-10 revenue, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `orderCount` | `number` |  |
| `periodStart` | `string` |  |

### RevenueContractFilterInput

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `status?` | `RevenueContractStatus` | draft, active, completed, cancelled Types: [`RevenueContractStatus`](#revenuecontractstatus) |
| `effectiveFrom?` | `string` | ISO date (YYYY-MM-DD) |
| `effectiveTo?` | `string` | ISO date (YYYY-MM-DD) |
| `search?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### RevenueContractOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `contractNumber` | `string` |  |
| `customerId` | `string` |  |
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `transactionPrice` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `status` | `RevenueContractStatus` | draft, active, completed, cancelled Types: [`RevenueContractStatus`](#revenuecontractstatus) |
| `effectiveDate` | `string` | ISO date (YYYY-MM-DD) |
| `obligations` | `Array<PerformanceObligationOutput>` | Types: [`PerformanceObligationOutput`](#performanceobligationoutput) |
| `totalRecognized` | `string` | Exact decimal string: total recognized across obligations |
| `deferredBalance` | `string` | Exact decimal string: transaction_price - total_recognized |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### RevenueContractStatus

Revenue contract lifecycle status (`RevenueContractStatus`); transitions are guarded.

```ts
type RevenueContractStatus = 'draft' | 'active' | 'completed' | 'cancelled'
```

One of: `'draft'`, `'active'`, `'completed'`, `'cancelled'`.

### RevenueEntryStatus

Revenue schedule entry status (`RevenueEntryStatus`).

```ts
type RevenueEntryStatus = 'deferred' | 'recognized'
```

One of: `'deferred'`, `'recognized'`.

### RevenueForecastOutput

| Field | Type | Description |
|---|---|---|
| `period` | `string` |  |
| `forecastedRevenue` | `number` | **Deprecated.** Use the `forecastedRevenueExact` twin; float money will be removed in 2.0. |
| `forecastedRevenueExact` | `string` | Exact base-10 forecasted revenue, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `lowerBound` | `number` | **Deprecated.** Use the `lowerBoundExact` twin; float money will be removed in 2.0. |
| `lowerBoundExact` | `string` | Exact base-10 lower bound, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `upperBound` | `number` | **Deprecated.** Use the `upperBoundExact` twin; float money will be removed in 2.0. |
| `upperBoundExact` | `string` | Exact base-10 upper bound, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `confidenceLevel` | `number` |  |
| `basedOnPeriods` | `number` |  |

### RevenueScheduleEntryOutput

| Field | Type | Description |
|---|---|---|
| `period` | `number` |  |
| `periodStart` | `string` | ISO date (YYYY-MM-DD): first day of the entry's month |
| `amount` | `string` | Exact decimal string |
| `status` | `RevenueEntryStatus` | deferred or recognized Types: [`RevenueEntryStatus`](#revenueentrystatus) |

### RevenueScheduleOutput

| Field | Type | Description |
|---|---|---|
| `obligationId` | `string` |  |
| `method` | `RecognitionMethodOutput` | point_in_time, ratable_over_time, milestone Types: [`RecognitionMethodOutput`](#recognitionmethodoutput) |
| `recognitionStart?` | `string` | ISO date (YYYY-MM-DD); set for ratable_over_time |
| `recognitionEnd?` | `string` | ISO date (YYYY-MM-DD); set for ratable_over_time |
| `entries` | `Array<RevenueScheduleEntryOutput>` | Types: [`RevenueScheduleEntryOutput`](#revenuescheduleentryoutput) |
| `totalAmount` | `string` | Exact decimal string |
| `recognizedTotal` | `string` | Exact decimal string: sum of recognized entries |
| `deferredTotal` | `string` | Exact decimal string: sum of deferred entries |

### ReviewFilterInput

| Field | Type | Description |
|---|---|---|
| `productId?` | `string` |  |
| `customerId?` | `string` |  |
| `status?` | `ReviewStatus` | Types: [`ReviewStatus`](#reviewstatus) |
| `minRating?` | `number` |  |
| `verifiedOnly?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### ReviewOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `productId` | `string` |  |
| `customerId` | `string` |  |
| `rating` | `number` |  |
| `title?` | `string` |  |
| `body?` | `string` |  |
| `status` | `ReviewStatus` | Types: [`ReviewStatus`](#reviewstatus) |
| `verifiedPurchase` | `boolean` |  |
| `helpfulCount` | `number` |  |
| `reportedCount` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ReviewStatus

Review moderation status, shared by outputs and inputs (case-insensitive on input).

```ts
type ReviewStatus = 'pending' | 'approved' | 'rejected' | 'flagged'
```

One of: `'pending'`, `'approved'`, `'rejected'`, `'flagged'`.

### ReviewSummaryOutput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `averageRating` | `number` |  |
| `totalReviews` | `number` |  |
| `ratingDistribution` | `Array<number>` | Counts for 1★, 2★, 3★, 4★, 5★ (index 0 = 1 star) |

### RewardFilterInput

| Field | Type | Description |
|---|---|---|
| `programId?` | `string` |  |
| `rewardType?` | `LoyaltyRewardType` | Types: [`LoyaltyRewardType`](#loyaltyrewardtype) |
| `isActive?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### RewardOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `programId` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `pointsCost` | `number` |  |
| `rewardType` | `LoyaltyRewardType` | Types: [`LoyaltyRewardType`](#loyaltyrewardtype) |
| `value?` | `string` |  |
| `isActive` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### RoundingMode

Conversion rounding mode (`StoreCurrencySettingsOutput.roundingMode`, `StoreCurrencySettingsInput.roundingMode`); an unrecognised input is refused with `VALIDATION`.

```ts
type RoundingMode = 'half_up' | 'half_down' | 'up' | 'down' | 'half_even'
```

One of: `'half_up'`, `'half_down'`, `'up'`, `'down'`, `'half_even'`.

### SalesSummaryOutput

| Field | Type | Description |
|---|---|---|
| `totalRevenue` | `number` | **Deprecated.** Use the `totalRevenueExact` twin; float money will be removed in 2.0. |
| `totalRevenueExact` | `string` | Exact base-10 total revenue, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `orderCount` | `number` |  |
| `averageOrderValue` | `number` | **Deprecated.** Use the `averageOrderValueExact` twin; float money will be removed in 2.0. |
| `averageOrderValueExact` | `string` | Exact base-10 average order value, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `itemsSold` | `number` |  |
| `uniqueCustomers` | `number` |  |

### SearchConfigFilterInput

| Field | Type | Description |
|---|---|---|
| `isActive?` | `boolean` |  |
| `name?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### SearchConfigOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `searchableFields` | `Array<SearchFieldOutput>` | Types: [`SearchFieldOutput`](#searchfieldoutput) |
| `facets` | `Array<FacetConfigOutput>` | Types: [`FacetConfigOutput`](#facetconfigoutput) |
| `synonyms` | `Array<SynonymGroupOutput>` | Types: [`SynonymGroupOutput`](#synonymgroupoutput) |
| `boostRules` | `Array<BoostRuleOutput>` | Types: [`BoostRuleOutput`](#boostruleoutput) |
| `isActive` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### SearchFieldInput

| Field | Type | Description |
|---|---|---|
| `fieldName` | `string` |  |
| `weight` | `number` |  |
| `tokenizer?` | `SearchTokenizer` | Snake-case tokenizer: `standard`, `ngram`, `edge`, `keyword` Types: [`SearchTokenizer`](#searchtokenizer) |
| `enabled?` | `boolean` |  |

### SearchFieldOutput

| Field | Type | Description |
|---|---|---|
| `fieldName` | `string` |  |
| `weight` | `number` |  |
| `tokenizer` | `SearchTokenizer` | Snake-case tokenizer Types: [`SearchTokenizer`](#searchtokenizer) |
| `enabled` | `boolean` |  |

### SearchTokenizer

Tokenizer strategy for a searchable field (`Tokenizer`).

```ts
type SearchTokenizer = 'standard' | 'ngram' | 'edge' | 'keyword'
```

One of: `'standard'`, `'ngram'`, `'edge'`, `'keyword'`.

### SegmentFilterInput

| Field | Type | Description |
|---|---|---|
| `segmentType?` | `SegmentType` | Types: [`SegmentType`](#segmenttype) |
| `name?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### SegmentMembershipOutput

| Field | Type | Description |
|---|---|---|
| `segmentId` | `string` |  |
| `customerId` | `string` |  |
| `joinedAt` | `string` |  |

### SegmentOperator

Segment rule comparison operator, shared by outputs and inputs (case-insensitive on input).

```ts
type SegmentOperator = 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte' | 'contains' | 'in' | 'between' | 'starts_with' | 'ends_with'
```

One of: `'eq'`, `'neq'`, `'gt'`, `'gte'`, `'lt'`, `'lte'`, `'contains'`, `'in'`, `'between'`, `'starts_with'`, `'ends_with'`.

### SegmentOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `segmentType` | `SegmentType` | Types: [`SegmentType`](#segmenttype) |
| `rules` | `Array<SegmentRuleOutput>` | Types: [`SegmentRuleOutput`](#segmentruleoutput) |
| `memberCount` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### SegmentRuleInput

| Field | Type | Description |
|---|---|---|
| `field` | `string` |  |
| `operator` | `SegmentOperator` | One of: eq, neq, gt, gte, lt, lte, contains, in, between, starts_with, ends_with Types: [`SegmentOperator`](#segmentoperator) |
| `value` | `string` |  |

### SegmentRuleOutput

| Field | Type | Description |
|---|---|---|
| `field` | `string` |  |
| `operator` | `SegmentOperator` | Types: [`SegmentOperator`](#segmentoperator) |
| `value` | `string` |  |

### SegmentType

Segment membership model, shared by outputs and inputs (case-insensitive on input).

```ts
type SegmentType = 'static' | 'dynamic'
```

One of: `'static'`, `'dynamic'`.

### SerialFilterInput

Optional filters for `Serials.list`. Every field is optional; an empty
object (or no argument) lists everything, unpaginated, as before.

| Field | Type | Description |
|---|---|---|
| `serial?` | `string` |  |
| `serialPrefix?` | `string` |  |
| `sku?` | `string` |  |
| `status?` | `SerialStatusInput` | The rendered form (`InService`) or the engine's snake_case (`in_service`). Types: [`SerialStatusInput`](#serialstatusinput) |
| `lotId?` | `string` |  |
| `lotNumber?` | `string` |  |
| `locationId?` | `number` |  |
| `ownerId?` | `string` |  |
| `hasWarranty?` | `boolean` |  |
| `manufacturedAfter?` | `string` | RFC 3339 timestamp. |
| `manufacturedBefore?` | `string` | RFC 3339 timestamp. |
| `soldAfter?` | `string` | RFC 3339 timestamp. |
| `soldBefore?` | `string` | RFC 3339 timestamp. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### SerialOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `serial` | `string` |  |
| `sku` | `string` |  |
| `lotId?` | `string` |  |
| `status` | `SerialStatus` | Types: [`SerialStatus`](#serialstatus) |
| `ownerId?` | `string` |  |
| `locationId?` | `number` |  |
| `createdAt` | `string` |  |

### SerialStatus

Serial-number status as rendered on `SerialOutput.status` (Rust `Debug` form).

```ts
type SerialStatus = 'InProduction' | 'Available' | 'Reserved' | 'Shipped' | 'Sold' | 'Returned' | 'InService' | 'InWarranty' | 'Quarantined' | 'Scrapped' | 'Recalled' | 'Lost' | 'Transferred'
```

One of: `'InProduction'`, `'Available'`, `'Reserved'`, `'Shipped'`, `'Sold'`, `'Returned'`, `'InService'`, `'InWarranty'`, `'Quarantined'`, `'Scrapped'`, `'Recalled'`, `'Lost'`, `'Transferred'`.

### SerialStatusInput

Serial-number status accepted by `SerialFilterInput.status`: the rendered form or the engine's snake_case.

```ts
type SerialStatusInput = SerialStatus | 'in_production' | 'available' | 'reserved' | 'shipped' | 'sold' | 'returned' | 'in_service' | 'in_warranty' | 'quarantined' | 'scrapped' | 'recalled' | 'lost' | 'transferred'
```

Types: [`SerialStatus`](#serialstatus)

### SetCartPaymentInput

| Field | Type | Description |
|---|---|---|
| `paymentMethod` | `string` |  |
| `paymentToken?` | `string` |  |

### SetCartShippingInput

| Field | Type | Description |
|---|---|---|
| `shippingAddress` | `CartAddressInput` | Types: [`CartAddressInput`](#cartaddressinput) |
| `shippingMethod?` | `string` |  |
| `shippingCarrier?` | `string` |  |
| `shippingAmount?` | `number` |  |
| `shippingAmountExact?` | `string` | Exact base-10 shipping amount. Takes precedence over `shipping_amount` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### SetExchangeRateInput

| Field | Type | Description |
|---|---|---|
| `baseCurrency` | `string` | Base currency code (e.g., "USD") |
| `quoteCurrency` | `string` | Quote currency code (e.g., "EUR") |
| `rate` | `number` | Exchange rate (e.g., 0.92 for USD to EUR) |
| `source?` | `string` | Optional source of the rate (e.g., "manual", "api") |

### SetItemCostInput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `costMethod?` | `CostMethodInput` | Types: [`CostMethodInput`](#costmethodinput) |
| `standardCost?` | `number` |  |
| `materialCost?` | `number` |  |
| `laborCost?` | `number` |  |
| `overheadCost?` | `number` |  |

### ShipmentFilterInput

Filter for `shipments.list()`

| Field | Type | Description |
|---|---|---|
| `orderId?` | `string` |  |
| `status?` | `ShipmentStatus \| 'readytoship' \| 'intransit' \| 'outfordelivery' \| 'canceled' \| 'onhold'` | Shipment status (`canceled` and the unseparated spellings are accepted too) Types: [`ShipmentStatus`](#shipmentstatus) |
| `carrier?` | `ShippingCarrierFilter` | Carrier Types: [`ShippingCarrierFilter`](#shippingcarrierfilter) |
| `trackingNumber?` | `string` |  |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |

### ShipmentMethod

Shipping service level as rendered on `ShipmentOutput.shippingMethod`.

```ts
type ShipmentMethod = 'standard' | 'express' | 'overnight' | 'two_day' | 'ground' | 'international' | 'same_day' | 'freight'
```

One of: `'standard'`, `'express'`, `'overnight'`, `'two_day'`, `'ground'`, `'international'`, `'same_day'`, `'freight'`.

### ShipmentMethodInput

Every spelling `CreateShipmentInput.shippingMethod` accepts; anything else is refused with `VALIDATION`.

```ts
type ShipmentMethodInput = ShipmentMethod | 'twoday' | 'sameday'
```

Types: [`ShipmentMethod`](#shipmentmethod)

### ShipmentOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `shipmentNumber` | `string` |  |
| `orderId` | `string` |  |
| `status` | `ShipmentStatus` | Types: [`ShipmentStatus`](#shipmentstatus) |
| `carrier` | `ShippingCarrier` | Types: [`ShippingCarrier`](#shippingcarrier) |
| `shippingMethod` | `ShipmentMethod` | Types: [`ShipmentMethod`](#shipmentmethod) |
| `trackingNumber?` | `string` |  |
| `trackingUrl?` | `string` |  |
| `recipientName` | `string` |  |
| `shippingAddress` | `string` |  |
| `version` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ShipmentStatus

Shipment lifecycle status (`ShipmentOutput.status`, `ShipmentFilterInput.status`).

```ts
type ShipmentStatus = 'pending' | 'processing' | 'ready_to_ship' | 'shipped' | 'in_transit' | 'out_for_delivery' | 'delivered' | 'failed' | 'returned' | 'cancelled' | 'on_hold'
```

One of: `'pending'`, `'processing'`, `'ready_to_ship'`, `'shipped'`, `'in_transit'`, `'out_for_delivery'`, `'delivered'`, `'failed'`, `'returned'`, `'cancelled'`, `'on_hold'`.

### ShippingCarrier

Carrier as rendered on `ShipmentOutput.carrier` (the engine renders the underscored spelling of multi-word carriers).

```ts
type ShippingCarrier = 'other' | 'ups' | 'fed_ex' | 'usps' | 'dhl' | 'on_trac' | 'laser_ship'
```

One of: `'other'`, `'ups'`, `'fed_ex'`, `'usps'`, `'dhl'`, `'on_trac'`, `'laser_ship'`.

### ShippingCarrierFilter

Carrier spellings `ShipmentFilterInput.carrier` accepts.

```ts
type ShippingCarrierFilter = ShippingCarrier | 'fedex' | 'ontrac' | 'lasership'
```

Types: [`ShippingCarrier`](#shippingcarrier)

### ShippingCarrierInput

Carriers `CreateShipmentInput.carrier` recognises; anything else is stored as `other`.

```ts
type ShippingCarrierInput = 'ups' | 'fedex' | 'usps' | 'dhl' | 'other'
```

One of: `'ups'`, `'fedex'`, `'usps'`, `'dhl'`, `'other'`.

### ShippingConditionInput

| Field | Type | Description |
|---|---|---|
| `minWeight?` | `string` | Exact decimal string |
| `maxWeight?` | `string` | Exact decimal string |
| `minPrice?` | `string` | Exact decimal string |
| `maxPrice?` | `string` | Exact decimal string |
| `rate` | `string` | Exact decimal string |

### ShippingConditionOutput

| Field | Type | Description |
|---|---|---|
| `minWeight?` | `string` | Exact decimal string |
| `maxWeight?` | `string` | Exact decimal string |
| `minPrice?` | `string` | Exact decimal string |
| `maxPrice?` | `string` | Exact decimal string |
| `rate` | `string` | Exact decimal string |

### ShippingMethodType

How a zone shipping method prices a shipment (`ShippingMethodType`).

```ts
type ShippingMethodType = 'flat' | 'weight_based' | 'price_based' | 'calculated' | 'free'
```

One of: `'flat'`, `'weight_based'`, `'price_based'`, `'calculated'`, `'free'`.

### ShippingRateOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `carrier` | `string` |  |
| `service` | `string` |  |
| `description?` | `string` |  |
| `price` | `number` | **Deprecated.** Use the `priceExact` twin; float money will be removed in 2.0. |
| `priceExact` | `string` | Exact base-10 price, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `currency` | `string` |  |
| `estimatedDays?` | `number` |  |

### ShippingZoneFilterInput

| Field | Type | Description |
|---|---|---|
| `country?` | `string` |  |
| `isActive?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### ShippingZoneOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `countries` | `Array<string>` |  |
| `regions` | `Array<string>` |  |
| `postalCodes` | `Array<string>` |  |
| `priority` | `number` |  |
| `isActive` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### SkipBillingCycleInput

| Field | Type | Description |
|---|---|---|
| `reason?` | `string` |  |

### StockLevelOutput

| Field | Type | Description |
|---|---|---|
| `sku` | `string` |  |
| `name` | `string` |  |
| `totalOnHand` | `string` |  |
| `totalAllocated` | `string` |  |
| `totalAvailable` | `string` |  |

### StockPolicy

What order creation does when a line cannot be fully reserved (`CreateOrderInput.stockPolicy`); hyphenated spellings are accepted too.

```ts
type StockPolicy = 'allow_backorder' | 'allow-backorder' | 'reject_if_insufficient' | 'reject-if-insufficient'
```

One of: `'allow_backorder'`, `'allow-backorder'`, `'reject_if_insufficient'`, `'reject-if-insufficient'`.

### StockSnapshotFilterInput

| Field | Type | Description |
|---|---|---|
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### StockSnapshotLineOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `stockSnapshotId` | `string` |  |
| `productId` | `string` |  |
| `sku` | `string` |  |
| `quantityOnHand` | `string` | Exact decimal string |
| `quantityAvailable` | `string` | Exact decimal string |
| `location?` | `string` |  |

### StockSnapshotOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `label?` | `string` |  |
| `totalSkus` | `string` |  |
| `totalUnits` | `string` | Exact decimal string |
| `lines` | `Array<StockSnapshotLineOutput>` | Types: [`StockSnapshotLineOutput`](#stocksnapshotlineoutput) |
| `capturedAt` | `string` |  |

### StoreCreditFilterInput

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `status?` | `StoreCreditStatus` | Types: [`StoreCreditStatus`](#storecreditstatus) |
| `reason?` | `StoreCreditReason` | Types: [`StoreCreditReason`](#storecreditreason) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### StoreCreditOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `customerId` | `string` |  |
| `originalBalance` | `string` | Exact decimal string |
| `currentBalance` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `status` | `StoreCreditStatus` | Types: [`StoreCreditStatus`](#storecreditstatus) |
| `reason` | `StoreCreditReason` | Types: [`StoreCreditReason`](#storecreditreason) |
| `referenceId?` | `string` |  |
| `note?` | `string` |  |
| `expiresAt?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### StoreCreditReason

Store credit reason, shared by outputs and inputs (case-insensitive on input).

```ts
type StoreCreditReason = 'return' | 'loyalty' | 'compensation' | 'promotion' | 'manual' | 'gift_card'
```

One of: `'return'`, `'loyalty'`, `'compensation'`, `'promotion'`, `'manual'`, `'gift_card'`.

### StoreCreditStatus

Store credit status, shared by outputs and inputs (case-insensitive on input).

```ts
type StoreCreditStatus = 'active' | 'depleted' | 'expired' | 'voided'
```

One of: `'active'`, `'depleted'`, `'expired'`, `'voided'`.

### StoreCreditTransactionOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `storeCreditId` | `string` |  |
| `amount` | `string` | Exact decimal string (positive = credit, negative = debit) |
| `balanceAfter` | `string` | Exact decimal string |
| `transactionType` | `StoreCreditTransactionType` | Types: [`StoreCreditTransactionType`](#storecredittransactiontype) |
| `referenceId?` | `string` |  |
| `createdAt` | `string` |  |

### StoreCreditTransactionType

Store credit ledger entry kind on `StoreCreditTransactionOutput.transactionType`.

```ts
type StoreCreditTransactionType = 'issue' | 'apply' | 'adjust' | 'void' | 'expire'
```

One of: `'issue'`, `'apply'`, `'adjust'`, `'void'`, `'expire'`.

### StoreCurrencySettingsInput

| Field | Type | Description |
|---|---|---|
| `baseCurrency` | `string` | Base currency for the store |
| `enabledCurrencies` | `Array<string>` | List of enabled currency codes |
| `autoConvert?` | `boolean` | Whether to auto-convert prices |
| `roundingMode?` | `RoundingMode` | Rounding mode; an unrecognised value is refused with `VALIDATION`. Types: [`RoundingMode`](#roundingmode) |

### StoreCurrencySettingsOutput

| Field | Type | Description |
|---|---|---|
| `baseCurrency` | `string` |  |
| `enabledCurrencies` | `Array<string>` |  |
| `autoConvert` | `boolean` |  |
| `roundingMode` | `RoundingMode` | Types: [`RoundingMode`](#roundingmode) |

### StrictEncryptionResultOutput

| Field | Type | Description |
|---|---|---|
| `payloadEncryptedJson` | `string` |  |
| `salt` | `Buffer` |  |
| `payloadPlainHash` | `Buffer` |  |
| `payloadCipherHash` | `Buffer` |  |

### StrictRecipientKeypairOutput

| Field | Type | Description |
|---|---|---|
| `kid` | `number` |  |
| `mlKem768PublicKey` | `Buffer` |  |
| `mlKem768Seed` | `Buffer` |  |

### StrictRecipientPrivateKeyInput

| Field | Type | Description |
|---|---|---|
| `mlKem768Seed` | `Buffer` |  |

### StrictRecipientPublicKeyInput

| Field | Type | Description |
|---|---|---|
| `kid` | `number` |  |
| `mlKem768PublicKey` | `Buffer` |  |

### StrictSigningKeypairOutput

| Field | Type | Description |
|---|---|---|
| `mlDsa65PublicKey` | `Buffer` |  |
| `mlDsa65Seed` | `Buffer` |  |

### SubscriptionBillingInterval

Billing interval as rendered on `SubscriptionPlanOutput` / `SubscriptionOutput` `.billingInterval`.

```ts
type SubscriptionBillingInterval = 'weekly' | 'bi-weekly' | 'monthly' | 'bi-monthly' | 'quarterly' | 'semi-annual' | 'yearly' | 'custom'
```

One of: `'weekly'`, `'bi-weekly'`, `'monthly'`, `'bi-monthly'`, `'quarterly'`, `'semi-annual'`, `'yearly'`, `'custom'`.

### SubscriptionBillingIntervalInput

Billing interval accepted on input (`CreateSubscriptionPlanInput`, `SubscriptionPlanFilterInput`); case-insensitive.

```ts
type SubscriptionBillingIntervalInput = 'weekly' | 'biweekly' | 'monthly' | 'bimonthly' | 'quarterly' | 'semiannual' | 'annual' | 'custom'
```

One of: `'weekly'`, `'biweekly'`, `'monthly'`, `'bimonthly'`, `'quarterly'`, `'semiannual'`, `'annual'`, `'custom'`.

### SubscriptionEventOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `subscriptionId` | `string` |  |
| `eventType` | `SubscriptionEventType` | Types: [`SubscriptionEventType`](#subscriptioneventtype) |
| `description` | `string` |  |
| `data?` | `string` |  |
| `triggeredBy?` | `string` |  |
| `createdAt` | `string` |  |

### SubscriptionEventType

Subscription event type as rendered on `SubscriptionEventOutput.eventType` (lower-cased Rust `Debug` form).

```ts
type SubscriptionEventType = 'created' | 'activated' | 'trialstarted' | 'trialended' | 'renewed' | 'paymentfailed' | 'paymentretrysucceeded' | 'paused' | 'resumed' | 'skipped' | 'cancelled' | 'expired' | 'planchanged' | 'itemsmodified' | 'quantitychanged' | 'addressupdated' | 'paymentmethodupdated' | 'discountapplied' | 'discountremoved' | 'refunded'
```

One of: `'created'`, `'activated'`, `'trialstarted'`, `'trialended'`, `'renewed'`, `'paymentfailed'`, `'paymentretrysucceeded'`, `'paused'`, `'resumed'`, `'skipped'`, `'cancelled'`, `'expired'`, `'planchanged'`, `'itemsmodified'`, `'quantitychanged'`, `'addressupdated'`, `'paymentmethodupdated'`, `'discountapplied'`, `'discountremoved'`, `'refunded'`.

### SubscriptionFilterInput

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `planId?` | `string` |  |
| `status?` | `SubscriptionStatus` | Types: [`SubscriptionStatus`](#subscriptionstatus) |
| `fromDate?` | `string` |  |
| `toDate?` | `string` |  |
| `search?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### SubscriptionOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `subscriptionNumber` | `string` |  |
| `customerId` | `string` |  |
| `planId` | `string` |  |
| `planName` | `string` |  |
| `status` | `SubscriptionStatus` | Types: [`SubscriptionStatus`](#subscriptionstatus) |
| `billingInterval` | `SubscriptionBillingInterval` | Types: [`SubscriptionBillingInterval`](#subscriptionbillinginterval) |
| `customIntervalDays?` | `number` |  |
| `price` | `number` | **Deprecated.** Use the `priceExact` twin; float money will be removed in 2.0. |
| `priceExact` | `string` | Exact base-10 price, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `currency` | `string` |  |
| `paymentMethodId?` | `string` |  |
| `startedAt` | `string` |  |
| `currentPeriodStart` | `string` |  |
| `currentPeriodEnd` | `string` |  |
| `nextBillingDate?` | `string` |  |
| `trialEndsAt?` | `string` |  |
| `pausedAt?` | `string` |  |
| `resumeAt?` | `string` |  |
| `cancelledAt?` | `string` |  |
| `endsAt?` | `string` |  |
| `billingCycleCount` | `number` |  |
| `failedPaymentAttempts` | `number` |  |
| `discountPercent?` | `number` |  |
| `discountAmount?` | `number` | **Deprecated.** Use the `discountAmountExact` twin; float money will be removed in 2.0. |
| `discountAmountExact?` | `string` | Exact base-10 discount amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `couponCode?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### SubscriptionPlanFilterInput

| Field | Type | Description |
|---|---|---|
| `status?` | `SubscriptionPlanStatus` | Types: [`SubscriptionPlanStatus`](#subscriptionplanstatus) |
| `billingInterval?` | `SubscriptionBillingIntervalInput` | Types: [`SubscriptionBillingIntervalInput`](#subscriptionbillingintervalinput) |
| `search?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### SubscriptionPlanOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `code` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `status` | `SubscriptionPlanStatus` | Types: [`SubscriptionPlanStatus`](#subscriptionplanstatus) |
| `billingInterval` | `SubscriptionBillingInterval` | Types: [`SubscriptionBillingInterval`](#subscriptionbillinginterval) |
| `customIntervalDays?` | `number` |  |
| `price` | `number` | **Deprecated.** Use the `priceExact` twin; float money will be removed in 2.0. |
| `priceExact` | `string` | Exact base-10 price, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `setupFee?` | `number` | **Deprecated.** Use the `setupFeeExact` twin; float money will be removed in 2.0. |
| `setupFeeExact?` | `string` | Exact base-10 setup fee, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `currency` | `string` |  |
| `trialDays` | `number` |  |
| `trialRequiresPaymentMethod` | `boolean` |  |
| `minCycles?` | `number` |  |
| `maxCycles?` | `number` |  |
| `discountPercent?` | `number` |  |
| `discountAmount?` | `number` | **Deprecated.** Use the `discountAmountExact` twin; float money will be removed in 2.0. |
| `discountAmountExact?` | `string` | Exact base-10 discount amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### SubscriptionPlanStatus

Subscription plan status (`SubscriptionPlanOutput.status`, `SubscriptionPlanFilterInput.status`).

```ts
type SubscriptionPlanStatus = 'draft' | 'active' | 'archived'
```

One of: `'draft'`, `'active'`, `'archived'`.

### SubscriptionStatus

Subscription status, shared by `SubscriptionOutput.status` and the update/filter inputs (case-insensitive on input).

```ts
type SubscriptionStatus = 'pending' | 'trial' | 'active' | 'paused' | 'past_due' | 'cancelled' | 'expired'
```

One of: `'pending'`, `'trial'`, `'active'`, `'paused'`, `'past_due'`, `'cancelled'`, `'expired'`.

### SupplierFilterInput

Filter for `purchaseOrders.listSuppliers()`

| Field | Type | Description |
|---|---|---|
| `name?` | `string` | Search by name |
| `country?` | `string` | Filter by country |
| `activeOnly?` | `boolean` | Only active suppliers |
| `limit?` | `number` | Page size (server default 100) |
| `offset?` | `number` |  |

### SupplierOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `supplierCode?` | `string` |  |
| `email?` | `string` |  |
| `phone?` | `string` |  |
| `isActive` | `boolean` |  |
| `createdAt` | `string` |  |

### SupplierSkuFilterInput

| Field | Type | Description |
|---|---|---|
| `supplierId?` | `string` |  |
| `productId?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### SupplierSkuOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `productId` | `string` |  |
| `supplierId` | `string` |  |
| `sku` | `string` |  |
| `unitCost?` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `minOrderQty?` | `string` | Exact decimal string |
| `leadTimeDays?` | `number` |  |
| `isPreferred` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### SynonymGroupInput

| Field | Type | Description |
|---|---|---|
| `canonical` | `string` |  |
| `synonyms` | `Array<string>` |  |

### SynonymGroupOutput

| Field | Type | Description |
|---|---|---|
| `canonical` | `string` |  |
| `synonyms` | `Array<string>` |  |

### TaxAddressInput

| Field | Type | Description |
|---|---|---|
| `line1?` | `string` |  |
| `line2?` | `string` |  |
| `city?` | `string` |  |
| `state?` | `string` |  |
| `postalCode?` | `string` |  |
| `country` | `string` |  |

### TaxBreakdownOutput

| Field | Type | Description |
|---|---|---|
| `jurisdictionId` | `string` |  |
| `jurisdictionName` | `string` |  |
| `taxType` | `TaxType` | Types: [`TaxType`](#taxtype) |
| `rateName` | `string` |  |
| `rate` | `number` |  |
| `taxableAmount` | `number` | **Deprecated.** Use the `taxableAmountExact` twin; float money will be removed in 2.0. |
| `taxableAmountExact` | `string` | Exact base-10 taxable amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `taxAmount` | `number` | **Deprecated.** Use the `taxAmountExact` twin; float money will be removed in 2.0. |
| `taxAmountExact` | `string` | Exact base-10 tax amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `isCompound` | `boolean` |  |

### TaxCalculationInput

| Field | Type | Description |
|---|---|---|
| `lineItems` | `Array<TaxLineItemInput>` | Types: [`TaxLineItemInput`](#taxlineiteminput) |
| `shippingAddress` | `TaxAddressInput` | Types: [`TaxAddressInput`](#taxaddressinput) |
| `billingAddress?` | `TaxAddressInput` | Types: [`TaxAddressInput`](#taxaddressinput) |
| `customerId?` | `string` |  |
| `shippingAmount?` | `number` |  |
| `currency?` | `string` |  |
| `transactionDate?` | `string` |  |
| `pricesIncludeTax?` | `boolean` |  |

### TaxCalculationMethod

Whether prices carry tax (`TaxSettingsOutput.calculationMethod`; on input, anything but `inclusive` means `exclusive`).

```ts
type TaxCalculationMethod = 'exclusive' | 'inclusive'
```

One of: `'exclusive'`, `'inclusive'`.

### TaxCalculationOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `totalTax` | `number` | **Deprecated.** Use the `totalTaxExact` twin; float money will be removed in 2.0. |
| `totalTaxExact` | `string` | Exact base-10 total tax, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `subtotal` | `number` | **Deprecated.** Use the `subtotalExact` twin; float money will be removed in 2.0. |
| `subtotalExact` | `string` | Exact base-10 subtotal, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `total` | `number` | **Deprecated.** Use the `totalExact` twin; float money will be removed in 2.0. |
| `totalExact` | `string` | Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `shippingTax` | `number` | **Deprecated.** Use the `shippingTaxExact` twin; float money will be removed in 2.0. |
| `shippingTaxExact` | `string` | Exact base-10 shipping tax, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `taxBreakdown` | `Array<TaxBreakdownOutput>` | Types: [`TaxBreakdownOutput`](#taxbreakdownoutput) |
| `lineItemTaxes` | `Array<LineItemTaxOutput>` | Types: [`LineItemTaxOutput`](#lineitemtaxoutput) |
| `exemptionsApplied` | `boolean` |  |
| `exemptionDetails?` | `ExemptionDetailsOutput` | Types: [`ExemptionDetailsOutput`](#exemptiondetailsoutput) |
| `jurisdictions` | `Array<JurisdictionSummaryOutput>` | Types: [`JurisdictionSummaryOutput`](#jurisdictionsummaryoutput) |
| `calculatedAt` | `string` |  |
| `isEstimate` | `boolean` |  |

### TaxCompoundMethod

How stacked taxes combine (`TaxSettingsOutput.compoundMethod`; on input,).

```ts
type TaxCompoundMethod = 'combined' | 'compound' | 'separate'
```

One of: `'combined'`, `'compound'`, `'separate'`.

### TaxDetailOutput

| Field | Type | Description |
|---|---|---|
| `taxType` | `TaxType` | Types: [`TaxType`](#taxtype) |
| `jurisdictionName` | `string` |  |
| `rate` | `number` |  |
| `amount` | `number` | **Deprecated.** Use the `amountExact` twin; float money will be removed in 2.0. |
| `amountExact` | `string` | Exact base-10 amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### TaxExemptionOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `customerId` | `string` |  |
| `exemptionType` | `TaxExemptionType` | Types: [`TaxExemptionType`](#taxexemptiontype) |
| `certificateNumber?` | `string` |  |
| `issuingAuthority?` | `string` |  |
| `jurisdictionIds` | `Array<string>` |  |
| `exemptCategories` | `ProductTaxCategory[]` | Types: [`ProductTaxCategory`](#producttaxcategory) |
| `effectiveFrom` | `string` |  |
| `expiresAt?` | `string` |  |
| `verified` | `boolean` |  |
| `verifiedAt?` | `string` |  |
| `notes?` | `string` |  |
| `active` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### TaxExemptionType

Tax exemption type as rendered on outputs (lower-cased Rust `Debug` form).

```ts
type TaxExemptionType = 'resale' | 'nonprofit' | 'government' | 'educational' | 'religious' | 'medical' | 'manufacturing' | 'agricultural' | 'export' | 'diplomatic' | 'other'
```

One of: `'resale'`, `'nonprofit'`, `'government'`, `'educational'`, `'religious'`, `'medical'`, `'manufacturing'`, `'agricultural'`, `'export'`, `'diplomatic'`, `'other'`.

### TaxExemptionTypeInput

Tax exemption type accepted by `CreateExemptionInput.exemptionType` (case-insensitive).

```ts
type TaxExemptionTypeInput = TaxExemptionType | 'non_profit'
```

Types: [`TaxExemptionType`](#taxexemptiontype)

### TaxJurisdictionLevel

Tax jurisdiction level, shared by outputs and inputs (case-insensitive on input).

```ts
type TaxJurisdictionLevel = 'country' | 'state' | 'county' | 'city' | 'district' | 'special'
```

One of: `'country'`, `'state'`, `'county'`, `'city'`, `'district'`, `'special'`.

### TaxJurisdictionOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `parentId?` | `string` |  |
| `name` | `string` |  |
| `code` | `string` |  |
| `level` | `TaxJurisdictionLevel` | Types: [`TaxJurisdictionLevel`](#taxjurisdictionlevel) |
| `countryCode` | `string` |  |
| `stateCode?` | `string` |  |
| `county?` | `string` |  |
| `city?` | `string` |  |
| `postalCodes` | `Array<string>` |  |
| `active` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### TaxLineItemInput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `sku?` | `string` |  |
| `productId?` | `string` |  |
| `quantity` | `number` |  |
| `unitPrice` | `number` |  |
| `discountAmount?` | `number` |  |
| `taxCategory?` | `ProductTaxCategory` | Types: [`ProductTaxCategory`](#producttaxcategory) |
| `taxCode?` | `string` |  |
| `description?` | `string` |  |

### TaxRateFilterInput

| Field | Type | Description |
|---|---|---|
| `jurisdictionId?` | `string` |  |
| `taxType?` | `TaxType` | Types: [`TaxType`](#taxtype) |
| `productCategory?` | `ProductTaxCategory` | Types: [`ProductTaxCategory`](#producttaxcategory) |
| `activeOnly?` | `boolean` |  |
| `effectiveDate?` | `string` |  |

### TaxRateOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `jurisdictionId` | `string` |  |
| `taxType` | `TaxType` | Types: [`TaxType`](#taxtype) |
| `productCategory` | `ProductTaxCategory` | Types: [`ProductTaxCategory`](#producttaxcategory) |
| `rate` | `number` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `isCompound` | `boolean` |  |
| `priority` | `number` |  |
| `thresholdMin?` | `number` | **Deprecated.** Use the `thresholdMinExact` twin; float money will be removed in 2.0. |
| `thresholdMinExact?` | `string` | Exact base-10 minimum amount at which the rate starts to apply. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `thresholdMax?` | `number` | **Deprecated.** Use the `thresholdMaxExact` twin; float money will be removed in 2.0. |
| `thresholdMaxExact?` | `string` | Exact base-10 cap on the amount this rate is charged against. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `fixedAmount?` | `number` | **Deprecated.** Use the `fixedAmountExact` twin; float money will be removed in 2.0. |
| `fixedAmountExact?` | `string` | Exact base-10 fixed amount, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `effectiveFrom` | `string` |  |
| `effectiveTo?` | `string` |  |
| `active` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### TaxSettingsInput

| Field | Type | Description |
|---|---|---|
| `enabled?` | `boolean` |  |
| `calculationMethod?` | `TaxCalculationMethod` | Types: [`TaxCalculationMethod`](#taxcalculationmethod) |
| `compoundMethod?` | `TaxCompoundMethod` | Types: [`TaxCompoundMethod`](#taxcompoundmethod) |
| `taxShipping?` | `boolean` |  |
| `taxHandling?` | `boolean` |  |
| `taxGiftWrap?` | `boolean` |  |
| `defaultProductCategory?` | `ProductTaxCategory` | Types: [`ProductTaxCategory`](#producttaxcategory) |
| `roundingMode?` | `string` |  |
| `decimalPlaces?` | `number` |  |
| `validateAddresses?` | `boolean` |  |
| `taxProvider?` | `string` |  |

### TaxSettingsOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `enabled` | `boolean` |  |
| `calculationMethod` | `TaxCalculationMethod` | Types: [`TaxCalculationMethod`](#taxcalculationmethod) |
| `compoundMethod` | `TaxCompoundMethod` | Types: [`TaxCompoundMethod`](#taxcompoundmethod) |
| `taxShipping` | `boolean` |  |
| `taxHandling` | `boolean` |  |
| `taxGiftWrap` | `boolean` |  |
| `defaultProductCategory` | `ProductTaxCategory` | Types: [`ProductTaxCategory`](#producttaxcategory) |
| `roundingMode` | `string` |  |
| `decimalPlaces` | `number` |  |
| `validateAddresses` | `boolean` |  |
| `taxProvider?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### TaxType

Tax type, shared by outputs and inputs (case-insensitive on input).

```ts
type TaxType = 'sales_tax' | 'vat' | 'gst' | 'hst' | 'pst' | 'qst' | 'consumption_tax' | 'custom'
```

One of: `'sales_tax'`, `'vat'`, `'gst'`, `'hst'`, `'pst'`, `'qst'`, `'consumption_tax'`, `'custom'`.

### ThreeWayMatchLineOutput

| Field | Type | Description |
|---|---|---|
| `poLineId?` | `string` |  |
| `billItemId` | `string` |  |
| `description` | `string` |  |
| `orderedQuantity?` | `string` | Exact decimal string |
| `orderedUnitCost?` | `string` | Exact decimal string |
| `receivedQuantity` | `string` | Exact decimal string |
| `billedQuantity` | `string` | Exact decimal string |
| `billedUnitCost` | `string` | Exact decimal string |
| `quantityVariance` | `string` | Exact decimal string: billed_quantity - received_quantity |
| `priceVariance` | `string` | Exact decimal string: billed_unit_cost - ordered_unit_cost |
| `matched` | `boolean` |  |
| `issues` | `Array<string>` |  |

### ThreeWayMatchOutput

| Field | Type | Description |
|---|---|---|
| `matchStatus` | `ThreeWayMatchStatus` | Overall status: not_required, pending, matched, variance Types: [`ThreeWayMatchStatus`](#threewaymatchstatus) |
| `varianceLineCount?` | `number` | Number of variance lines (set when match_status is "variance") |
| `tolerancePercent` | `string` | Tolerance applied, as an exact decimal string percentage (e.g. "5") |
| `lines` | `Array<ThreeWayMatchLineOutput>` | Types: [`ThreeWayMatchLineOutput`](#threewaymatchlineoutput) |

### ThreeWayMatchStatus

Three-way match outcome on `ThreeWayMatchOutput.matchStatus`.

```ts
type ThreeWayMatchStatus = 'not_required' | 'pending' | 'matched' | 'variance' | 'unknown'
```

One of: `'not_required'`, `'pending'`, `'matched'`, `'variance'`, `'unknown'`.

### TopCustomerOutput

| Field | Type | Description |
|---|---|---|
| `customerId` | `string` |  |
| `name` | `string` |  |
| `email` | `string` |  |
| `orderCount` | `number` |  |
| `totalSpent` | `number` | **Deprecated.** Use the `totalSpentExact` twin; float money will be removed in 2.0. |
| `totalSpentExact` | `string` | Exact base-10 total spent, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `averageOrderValue` | `number` | **Deprecated.** Use the `averageOrderValueExact` twin; float money will be removed in 2.0. |
| `averageOrderValueExact` | `string` | Exact base-10 average order value, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### TopProductOutput

| Field | Type | Description |
|---|---|---|
| `productId?` | `string` |  |
| `sku` | `string` |  |
| `name` | `string` |  |
| `unitsSold` | `number` |  |
| `revenue` | `number` | **Deprecated.** Use the `revenueExact` twin; float money will be removed in 2.0. |
| `revenueExact` | `string` | Exact base-10 revenue, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `orderCount` | `number` |  |

### TopologySnapshotFilterInput

| Field | Type | Description |
|---|---|---|
| `health?` | `HealthGrade` | Snake-case health grade: `unknown`, `healthy`, `degraded`, `critical` Types: [`HealthGrade`](#healthgrade) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### TopologySnapshotOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `channelsTotal` | `string` |  |
| `channelsActive` | `string` |  |
| `warehousesTotal` | `string` |  |
| `productsTotal` | `string` |  |
| `openOrders` | `string` |  |
| `health` | `HealthGrade` | Snake-case health grade Types: [`HealthGrade`](#healthgrade) |
| `signals` | `string` | JSON string |
| `capturedAt` | `string` |  |

### TransferOrderFilterInput

| Field | Type | Description |
|---|---|---|
| `status?` | `TransferOrderStatus` | draft, pending, in_transit, partially_received, received, cancelled Types: [`TransferOrderStatus`](#transferorderstatus) |
| `sourceWarehouseId?` | `string` |  |
| `destinationWarehouseId?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### TransferOrderItemOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `transferOrderId` | `string` |  |
| `productId` | `string` |  |
| `sku` | `string` |  |
| `quantity` | `string` | Exact decimal string |
| `quantityShipped` | `string` | Exact decimal string |
| `quantityReceived` | `string` | Exact decimal string |

### TransferOrderOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `number` | `string` |  |
| `sourceWarehouseId` | `string` |  |
| `destinationWarehouseId` | `string` |  |
| `status` | `TransferOrderStatus` | draft, pending, in_transit, partially_received, received, cancelled Types: [`TransferOrderStatus`](#transferorderstatus) |
| `items` | `Array<TransferOrderItemOutput>` | Types: [`TransferOrderItemOutput`](#transferorderitemoutput) |
| `expectedAt?` | `string` | RFC 3339 timestamp |
| `shippedAt?` | `string` | RFC 3339 timestamp |
| `receivedAt?` | `string` | RFC 3339 timestamp |
| `notes?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### TransferOrderStatus

Transfer order lifecycle status (`TransferOrderStatus`).

```ts
type TransferOrderStatus = 'draft' | 'pending' | 'in_transit' | 'partially_received' | 'received' | 'cancelled'
```

One of: `'draft'`, `'pending'`, `'in_transit'`, `'partially_received'`, `'received'`, `'cancelled'`.

### TrialBalanceOutput

| Field | Type | Description |
|---|---|---|
| `asOfDate` | `string` |  |
| `totalDebits` | `number` | **Deprecated.** Use the `totalDebitsExact` twin; float money will be removed in 2.0. |
| `totalDebitsExact` | `string` | Exact base-10 total debits, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `totalCredits` | `number` | **Deprecated.** Use the `totalCreditsExact` twin; float money will be removed in 2.0. |
| `totalCreditsExact` | `string` | Exact base-10 total credits, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `isBalanced` | `boolean` |  |

### UnitClassFilterInput

Window for `listClasses()`; omit for every class.

| Field | Type | Description |
|---|---|---|
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### UnitClassOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `baseUomId?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### UnitConversionRuleFilterInput

Filter for `listRules()`; omit for every rule.

| Field | Type | Description |
|---|---|---|
| `ruleType?` | `ConversionRuleType` | SYSTEM or SKU Types: [`ConversionRuleType`](#conversionruletype) |
| `productId?` | `string` | Only rules scoped to this product (SKU rules) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### UnitConversionRuleOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `ruleType` | `ConversionRuleType` | SYSTEM or SKU Types: [`ConversionRuleType`](#conversionruletype) |
| `productId?` | `string` |  |
| `fromUomId` | `string` |  |
| `toUomId` | `string` |  |
| `factor` | `string` | Exact decimal string |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### UnitOfMeasureFilterInput

| Field | Type | Description |
|---|---|---|
| `classId?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### UnitOfMeasureOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `unitClassId` | `string` |  |
| `name` | `string` |  |
| `abbreviation` | `string` |  |
| `factor` | `string` | Exact decimal string |
| `isBase` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### UpdateAgentIdentityInput

| Field | Type | Description |
|---|---|---|
| `agentUri?` | `string` |  |
| `agentWallet?` | `string` |  |
| `ownerAddress?` | `string` |  |
| `agentCardId?` | `string` |  |
| `registration?` | `string` | JSON-encoded registration document |
| `registrationHash?` | `string` |  |
| `walletProofType?` | `AgentWalletProofTypeInput` | Snake-case proof type Types: [`AgentWalletProofTypeInput`](#agentwalletprooftypeinput) |
| `walletProof?` | `string` |  |
| `walletProofChainId?` | `string` | Chain id as a decimal string |
| `walletProofDeadline?` | `string` | RFC3339 timestamp |
| `active?` | `boolean` |  |

### UpdateCartInput

| Field | Type | Description |
|---|---|---|
| `customerEmail?` | `string` |  |
| `customerPhone?` | `string` |  |
| `customerName?` | `string` |  |
| `shippingMethod?` | `string` |  |
| `couponCode?` | `string` |  |
| `notes?` | `string` |  |

### UpdateCartItemInput

| Field | Type | Description |
|---|---|---|
| `quantity?` | `number` |  |
| `unitPrice?` | `number` |  |
| `unitPriceExact?` | `string` | Exact base-10 unit price. Takes precedence over `unit_price` when present. _Exact money: a base-10 decimal string; prefer it over any float twin._ |

### UpdateChannelInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `integration?` | `string` |  |
| `status?` | `ChannelStatus` | active, paused, deleted Types: [`ChannelStatus`](#channelstatus) |
| `defaultWarehouseId?` | `string` |  |
| `tags?` | `Array<string>` |  |
| `metadata?` | `string` | Metadata as JSON |

### UpdateCompanyInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `reference?` | `string` |  |
| `email?` | `string` |  |
| `phone?` | `string` |  |
| `currency?` | `string` | ISO 4217 currency code |
| `paymentTermsDays?` | `number` |  |
| `status?` | `CompanyStatus` | active, inactive Types: [`CompanyStatus`](#companystatus) |
| `tags?` | `Array<string>` |  |
| `metadata?` | `string` | Metadata as JSON |

### UpdateCustomObjectInput

| Field | Type | Description |
|---|---|---|
| `handle?` | `string` |  |
| `ownerType?` | `string` |  |
| `ownerId?` | `string` |  |
| `valuesJson?` | `string` | JSON string representing record values (must be an object). |

### UpdateCustomObjectTypeInput

| Field | Type | Description |
|---|---|---|
| `displayName?` | `string` |  |
| `description?` | `string` |  |
| `fields?` | `Array<CustomFieldDefinitionInput>` | Types: [`CustomFieldDefinitionInput`](#customfielddefinitioninput) |

### UpdateCustomerInput

| Field | Type | Description |
|---|---|---|
| `email?` | `string` |  |
| `firstName?` | `string` |  |
| `lastName?` | `string` |  |
| `phone?` | `string` |  |
| `status?` | `CustomerStatus` | Types: [`CustomerStatus`](#customerstatus) |
| `acceptsMarketing?` | `boolean` |  |
| `tags?` | `Array<string>` |  |
| `metadata?` | `CustomerMetadata` | Types: [`CustomerMetadata`](#customermetadata) |

### UpdateFixedAssetInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `description?` | `string` |  |
| `category?` | `FixedAssetCategory` | Types: [`FixedAssetCategory`](#fixedassetcategory) |
| `salvageValue?` | `string` | Exact decimal string |
| `usefulLifeMonths?` | `number` |  |
| `inServiceDate?` | `string` | ISO date (YYYY-MM-DD) |
| `locationId?` | `string` |  |
| `assetAccountId?` | `string` |  |
| `accumulatedDepreciationAccountId?` | `string` |  |
| `depreciationExpenseAccountId?` | `string` |  |

### UpdateFraudRuleInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `description?` | `string` |  |
| `threshold?` | `number` |  |
| `action?` | `FraudDecision` | Snake-case decision Types: [`FraudDecision`](#frauddecision) |
| `enabled?` | `boolean` |  |

### UpdateGiftCardInput

| Field | Type | Description |
|---|---|---|
| `status?` | `GiftCardStatus` | Types: [`GiftCardStatus`](#giftcardstatus) |
| `recipientEmail?` | `string` |  |

### UpdateIntegrationFieldMappingInput

| Field | Type | Description |
|---|---|---|
| `destinationField?` | `string` |  |
| `template?` | `string` |  |
| `transform?` | `FieldTransform` | Types: [`FieldTransform`](#fieldtransform) |
| `fallback?` | `string` |  |
| `isActive?` | `boolean` |  |

### UpdateIntegrationMappingInput

| Field | Type | Description |
|---|---|---|
| `internalValue?` | `string` |  |
| `isActive?` | `boolean` |  |

### UpdatePriceLevelInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `description?` | `string` |  |
| `adjustmentType?` | `PriceAdjustmentType` | none, percentage_discount, percentage_markup Types: [`PriceAdjustmentType`](#priceadjustmenttype) |
| `adjustmentValue?` | `string` | Percentage as exact decimal string |
| `isActive?` | `boolean` |  |

### UpdatePriceScheduleInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `code?` | `string` |  |
| `startsAt?` | `string` | RFC 3339 timestamp |
| `endsAt?` | `string` | RFC 3339 timestamp |
| `isActive?` | `boolean` |  |
| `priority?` | `number` |  |

### UpdateProductInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `slug?` | `string` |  |
| `description?` | `string` |  |
| `status?` | `ProductStatus` | Types: [`ProductStatus`](#productstatus) |

### UpdateProductionBatchInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `vendorId?` | `string` |  |
| `status?` | `ProductionBatchStatus` | planned, in_progress, completed, cancelled Types: [`ProductionBatchStatus`](#productionbatchstatus) |
| `notes?` | `string` |  |
| `scheduledStart?` | `string` | RFC 3339 timestamp |
| `scheduledEnd?` | `string` | RFC 3339 timestamp |

### UpdatePromotionInput

Input for updating a promotion

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `description?` | `string` |  |
| `internalNotes?` | `string` |  |
| `status?` | `PromotionStatus` | Types: [`PromotionStatus`](#promotionstatus) |
| `percentageOff?` | `number` |  |
| `fixedAmountOff?` | `number` |  |
| `maxDiscountAmount?` | `number` |  |
| `startsAt?` | `string` |  |
| `endsAt?` | `string` |  |
| `totalUsageLimit?` | `number` |  |
| `perCustomerLimit?` | `number` |  |
| `priority?` | `number` |  |

### UpdateRevenueContractInput

| Field | Type | Description |
|---|---|---|
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `status?` | `RevenueContractStatus` | draft, active, completed, cancelled (transition-guarded) Types: [`RevenueContractStatus`](#revenuecontractstatus) |
| `effectiveDate?` | `string` | ISO date (YYYY-MM-DD) |

### UpdateReviewInput

| Field | Type | Description |
|---|---|---|
| `rating?` | `number` |  |
| `title?` | `string` |  |
| `body?` | `string` |  |
| `status?` | `ReviewStatus` | Moderation status: pending, approved, rejected, flagged Types: [`ReviewStatus`](#reviewstatus) |

### UpdateSearchConfigInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `description?` | `string` |  |
| `searchableFields?` | `Array<SearchFieldInput>` | Types: [`SearchFieldInput`](#searchfieldinput) |
| `facets?` | `Array<FacetConfigInput>` | Types: [`FacetConfigInput`](#facetconfiginput) |
| `synonyms?` | `Array<SynonymGroupInput>` | Types: [`SynonymGroupInput`](#synonymgroupinput) |
| `boostRules?` | `Array<BoostRuleInput>` | Types: [`BoostRuleInput`](#boostruleinput) |
| `isActive?` | `boolean` |  |

### UpdateSegmentInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `description?` | `string` |  |
| `rules?` | `Array<SegmentRuleInput>` | Types: [`SegmentRuleInput`](#segmentruleinput) |

### UpdateShippingZoneInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `countries?` | `Array<string>` |  |
| `regions?` | `Array<string>` |  |
| `postalCodes?` | `Array<string>` |  |
| `priority?` | `number` |  |
| `isActive?` | `boolean` |  |

### UpdateSubscriptionInput

| Field | Type | Description |
|---|---|---|
| `status?` | `SubscriptionStatus` | Types: [`SubscriptionStatus`](#subscriptionstatus) |
| `price?` | `number` |  |
| `paymentMethodId?` | `string` |  |
| `nextBillingDate?` | `string` |  |
| `discountPercent?` | `number` |  |
| `discountAmount?` | `number` |  |
| `couponCode?` | `string` |  |

### UpdateSubscriptionPlanInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `description?` | `string` |  |
| `price?` | `number` |  |
| `setupFee?` | `number` |  |
| `trialDays?` | `number` |  |
| `trialRequiresPaymentMethod?` | `boolean` |  |
| `minCycles?` | `number` |  |
| `maxCycles?` | `number` |  |
| `discountPercent?` | `number` |  |
| `discountAmount?` | `number` |  |

### UpdateSupplierSkuInput

| Field | Type | Description |
|---|---|---|
| `sku?` | `string` |  |
| `unitCost?` | `string` | Exact decimal string |
| `currency?` | `string` | Currency code, e.g. "USD" |
| `minOrderQty?` | `string` | Exact decimal string |
| `leadTimeDays?` | `number` |  |
| `isPreferred?` | `boolean` |  |

### UpdateWishlistInput

| Field | Type | Description |
|---|---|---|
| `name?` | `string` |  |
| `isPublic?` | `boolean` |  |

### UsStateTaxInfoOutput

| Field | Type | Description |
|---|---|---|
| `stateCode` | `string` |  |
| `stateName` | `string` |  |
| `stateRate` | `number` |  |
| `hasLocalTaxes` | `boolean` |  |
| `originBased` | `boolean` |  |
| `taxShipping` | `boolean` |  |
| `taxClothing` | `boolean` |  |
| `taxFood` | `boolean` |  |
| `taxDigital` | `boolean` |  |

### ValidationSummaryOutput

| Field | Type | Description |
|---|---|---|
| `count` | `string` | Count as a decimal string |
| `averageResponse` | `number` |  |

### VectorEntityType

Entity type accepted by `VectorSearch.clear` (case-insensitive).

```ts
type VectorEntityType = 'product' | 'products' | 'customer' | 'customers' | 'order' | 'orders' | 'inventory_item' | 'inventory' | 'inventory_items'
```

One of: `'product'`, `'products'`, `'customer'`, `'customers'`, `'order'`, `'orders'`, `'inventory_item'`, `'inventory'`, `'inventory_items'`.

### VectorSearchResultOutput

Kept for the generated declarations; the search path returns `serde_json::Value`.

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `distance` | `number` |  |
| `score` | `number` |  |

### VendorCreditApplicationOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `vendorCreditId` | `string` |  |
| `targetType` | `VendorCreditTargetType` | bill or payment_obligation Types: [`VendorCreditTargetType`](#vendorcredittargettype) |
| `targetId` | `string` |  |
| `amount` | `string` | Exact decimal string |
| `reversed` | `boolean` |  |
| `createdAt` | `string` |  |

### VendorCreditFilterInput

| Field | Type | Description |
|---|---|---|
| `supplierId?` | `string` |  |
| `status?` | `VendorCreditStatus` | open, applied, cancelled Types: [`VendorCreditStatus`](#vendorcreditstatus) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### VendorCreditOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `number` | `string` |  |
| `supplierId` | `string` |  |
| `vendorReturnId?` | `string` |  |
| `amount` | `string` | Exact decimal string |
| `remaining` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `status` | `VendorCreditStatus` | open, applied, cancelled Types: [`VendorCreditStatus`](#vendorcreditstatus) |
| `memo?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### VendorCreditStatus

Vendor credit lifecycle status (`VendorCreditStatus`).

```ts
type VendorCreditStatus = 'open' | 'applied' | 'cancelled'
```

One of: `'open'`, `'applied'`, `'cancelled'`.

### VendorCreditTargetType

What a vendor credit application is applied against (`VendorCreditTargetType`).

```ts
type VendorCreditTargetType = 'bill' | 'payment_obligation'
```

One of: `'bill'`, `'payment_obligation'`.

### VendorReturnFilterInput

| Field | Type | Description |
|---|---|---|
| `supplierId?` | `string` |  |
| `status?` | `VendorReturnStatus` | Snake-case status Types: [`VendorReturnStatus`](#vendorreturnstatus) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### VendorReturnItemOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `vendorReturnId` | `string` |  |
| `productId` | `string` |  |
| `sku` | `string` |  |
| `quantity` | `string` | Exact decimal string |
| `unitCost` | `string` | Exact decimal string |
| `lineTotal` | `string` | Exact decimal string |
| `reason` | `VendorReturnReason` | Snake-case reason Types: [`VendorReturnReason`](#vendorreturnreason) |

### VendorReturnOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `number` | `string` |  |
| `supplierId` | `string` |  |
| `purchaseOrderId?` | `string` |  |
| `status` | `VendorReturnStatus` | Snake-case status Types: [`VendorReturnStatus`](#vendorreturnstatus) |
| `currency` | `string` |  |
| `items` | `Array<VendorReturnItemOutput>` | Types: [`VendorReturnItemOutput`](#vendorreturnitemoutput) |
| `totalCredit` | `string` | Exact decimal string |
| `creditGenerated` | `boolean` |  |
| `notes?` | `string` |  |
| `processedAt?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### VendorReturnReason

Reason a line is returned to the vendor (`VendorReturnReason`).

```ts
type VendorReturnReason = 'defective' | 'overage' | 'wrong_item' | 'other'
```

One of: `'defective'`, `'overage'`, `'wrong_item'`, `'other'`.

### VendorReturnStatus

Vendor return lifecycle status (`VendorReturnStatus`).

```ts
type VendorReturnStatus = 'draft' | 'pending' | 'processed' | 'cancelled'
```

One of: `'draft'`, `'pending'`, `'processed'`, `'cancelled'`.

### VesHashDomain

Hash domain accepted by `domainHash` (exact, upper-case).

```ts
type VesHashDomain = 'PAYLOAD_PLAIN' | 'PAYLOAD_AAD' | 'PAYLOAD_CIPHER' | 'RECIPIENTS' | 'EVENTSIG' | 'LEAF' | 'NODE' | 'PAD_LEAF' | 'STREAM' | 'RECEIPT'
```

One of: `'PAYLOAD_PLAIN'`, `'PAYLOAD_AAD'`, `'PAYLOAD_CIPHER'`, `'RECIPIENTS'`, `'EVENTSIG'`, `'LEAF'`, `'NODE'`, `'PAD_LEAF'`, `'STREAM'`, `'RECEIPT'`.

### WarehouseFilterInput

Optional filters for `Warehouse.listWarehouses`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `warehouseType?` | `WarehouseTypeFilter` | The rendered form (`ThirdParty`) or the engine's snake_case (`third_party`). Types: [`WarehouseTypeFilter`](#warehousetypefilter) |
| `isActive?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### WarehouseLocationType

Warehouse location type as rendered on `LocationOutput.locationType` (Rust `Debug` form).

```ts
type WarehouseLocationType = 'Bulk' | 'Pick' | 'Staging' | 'Receiving' | 'Shipping' | 'Quarantine' | 'Returns' | 'Production' | 'Packing' | 'CrossDock'
```

One of: `'Bulk'`, `'Pick'`, `'Staging'`, `'Receiving'`, `'Shipping'`, `'Quarantine'`, `'Returns'`, `'Production'`, `'Packing'`, `'CrossDock'`.

Types: [`Receiving`](#commercereceiving), [`Returns`](#commercereturns)

### WarehouseLocationTypeInput

Warehouse location type accepted by `CreateLocationInput.locationType` (case-insensitive).

```ts
type WarehouseLocationTypeInput = 'pick' | 'bulk' | 'receiving' | 'shipping' | 'staging' | 'quarantine' | 'returns'
```

One of: `'pick'`, `'bulk'`, `'receiving'`, `'shipping'`, `'staging'`, `'quarantine'`, `'returns'`.

### WarehouseOutput

| Field | Type | Description |
|---|---|---|
| `id` | `number` |  |
| `code` | `string` |  |
| `name` | `string` |  |
| `warehouseType` | `WarehouseType` | Types: [`WarehouseType`](#warehousetype) |
| `isActive` | `boolean` |  |
| `timezone?` | `string` |  |
| `createdAt` | `string` |  |

### WarehouseType

Warehouse type as rendered on `WarehouseOutput.warehouseType` (Rust `Debug` form).

```ts
type WarehouseType = 'Distribution' | 'Manufacturing' | 'Retail' | 'ThirdParty' | 'Consignment' | 'Returns'
```

One of: `'Distribution'`, `'Manufacturing'`, `'Retail'`, `'ThirdParty'`, `'Consignment'`, `'Returns'`.

Types: [`Returns`](#commercereturns)

### WarehouseTypeFilter

Warehouse type accepted by `WarehouseFilterInput.warehouseType`: the rendered form or the engine's snake_case (strict).

```ts
type WarehouseTypeFilter = WarehouseType | 'distribution' | 'manufacturing' | 'retail' | 'third_party' | 'thirdparty' | 'consignment' | 'returns'
```

Types: [`WarehouseType`](#warehousetype)

### WarehouseTypeInput

Warehouse type accepted by `CreateWarehouseInput.warehouseType` (case-insensitive).

```ts
type WarehouseTypeInput = 'distribution' | 'manufacturing' | 'retail' | 'thirdparty' | 'third_party'
```

One of: `'distribution'`, `'manufacturing'`, `'retail'`, `'thirdparty'`, `'third_party'`.

### WarrantyClaimOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `claimNumber` | `string` |  |
| `warrantyId` | `string` |  |
| `status` | `WarrantyClaimStatus` | Types: [`WarrantyClaimStatus`](#warrantyclaimstatus) |
| `issueDescription` | `string` |  |
| `resolution` | `WarrantyClaimResolution` | Types: [`WarrantyClaimResolution`](#warrantyclaimresolution) |
| `createdAt` | `string` |  |

### WarrantyClaimResolution

How a warranty claim was resolved (`WarrantyClaimOutput.resolution`).

```ts
type WarrantyClaimResolution = 'none' | 'repair' | 'replacement' | 'refund' | 'store_credit' | 'denied'
```

One of: `'none'`, `'repair'`, `'replacement'`, `'refund'`, `'store_credit'`, `'denied'`.

### WarrantyClaimResolutionInput

Resolutions `warranties.completeClaim` accepts; anything else records `none`.

```ts
type WarrantyClaimResolutionInput = WarrantyClaimResolution | 'storecredit'
```

Types: [`WarrantyClaimResolution`](#warrantyclaimresolution)

### WarrantyClaimStatus

Warranty claim status (`WarrantyClaimOutput.status`).

```ts
type WarrantyClaimStatus = 'submitted' | 'under_review' | 'info_requested' | 'approved' | 'denied' | 'in_progress' | 'completed' | 'cancelled'
```

One of: `'submitted'`, `'under_review'`, `'info_requested'`, `'approved'`, `'denied'`, `'in_progress'`, `'completed'`, `'cancelled'`.

### WarrantyFilterInput

Filter for `warranties.list()`

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `orderId?` | `string` |  |
| `productId?` | `string` |  |
| `sku?` | `string` |  |
| `serialNumber?` | `string` |  |
| `status?` | `WarrantyStatus` | Warranty status Types: [`WarrantyStatus`](#warrantystatus) |
| `warrantyType?` | `WarrantyType` | Warranty tier Types: [`WarrantyType`](#warrantytype) |
| `activeOnly?` | `boolean` | Only warranties that have not expired |
| `expiringWithinDays?` | `number` | Only warranties expiring within this many days |
| `limit?` | `number` | Page size |
| `offset?` | `number` |  |

### WarrantyOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `warrantyNumber` | `string` |  |
| `customerId` | `string` |  |
| `productId?` | `string` |  |
| `orderId?` | `string` |  |
| `status` | `WarrantyStatus` | Types: [`WarrantyStatus`](#warrantystatus) |
| `warrantyType` | `WarrantyType` | Types: [`WarrantyType`](#warrantytype) |
| `startDate` | `string` |  |
| `endDate` | `string` |  |
| `createdAt` | `string` |  |

### WarrantyStatus

Warranty status (`WarrantyOutput.status`, `WarrantyFilterInput.status`).

```ts
type WarrantyStatus = 'active' | 'expired' | 'voided' | 'transferred'
```

One of: `'active'`, `'expired'`, `'voided'`, `'transferred'`.

### WarrantyType

Warranty tier as rendered on `WarrantyOutput.warrantyType` and accepted by `WarrantyFilterInput.warrantyType`.

```ts
type WarrantyType = 'standard' | 'extended' | 'limited' | 'lifetime' | 'accidental_damage' | 'comprehensive'
```

One of: `'standard'`, `'extended'`, `'limited'`, `'lifetime'`, `'accidental_damage'`, `'comprehensive'`.

### WarrantyTypeInput

Tiers `CreateWarrantyInput.warrantyType` recognises; anything else uses the engine default (`standard`).

```ts
type WarrantyTypeInput = 'standard' | 'extended' | 'limited' | 'lifetime'
```

One of: `'standard'`, `'extended'`, `'limited'`, `'lifetime'`.

### WaveFilterInput

Optional filters for `Fulfillment.listWaves`. No argument lists all.

| Field | Type | Description |
|---|---|---|
| `warehouseId?` | `number` |  |
| `status?` | `WaveStatusInput` | The rendered form (`InProgress`) or the engine's snake_case (`in_progress`). Types: [`WaveStatusInput`](#wavestatusinput) |
| `fromDate?` | `string` | RFC 3339 timestamp. |
| `toDate?` | `string` | RFC 3339 timestamp. |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### WaveOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `waveNumber` | `string` |  |
| `warehouseId` | `number` |  |
| `orderCount` | `number` |  |
| `status` | `WaveStatus` | Types: [`WaveStatus`](#wavestatus) |
| `createdAt` | `string` |  |

### WaveStatus

Wave status as rendered on `WaveOutput.status` (Rust `Debug` form).

```ts
type WaveStatus = 'Draft' | 'Released' | 'InProgress' | 'Completed' | 'Cancelled'
```

One of: `'Draft'`, `'Released'`, `'InProgress'`, `'Completed'`, `'Cancelled'`.

### WaveStatusInput

Wave status accepted by `WaveFilterInput.status`: the rendered form or the engine's snake_case (strict).

```ts
type WaveStatusInput = WaveStatus | 'draft' | 'released' | 'in_progress' | 'completed' | 'cancelled' | 'canceled'
```

Types: [`WaveStatus`](#wavestatus)

### WebhookOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `url` | `string` |  |
| `hasSecret` | `boolean` |  |
| `eventTypes` | `Array<string>` |  |
| `active` | `boolean` |  |
| `createdAt` | `string` |  |

### WishlistFilterInput

| Field | Type | Description |
|---|---|---|
| `customerId?` | `string` |  |
| `isPublic?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### WishlistItemOutput

| Field | Type | Description |
|---|---|---|
| `productId` | `string` |  |
| `variantId?` | `string` |  |
| `addedAt` | `string` |  |
| `note?` | `string` |  |
| `quantity` | `number` |  |
| `priority?` | `number` |  |

### WishlistOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `customerId` | `string` |  |
| `name` | `string` |  |
| `isPublic` | `boolean` |  |
| `items` | `Array<WishlistItemOutput>` | Types: [`WishlistItemOutput`](#wishlistitemoutput) |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### WorkOrderFilterInput

Filter for `workOrders.list()`

| Field | Type | Description |
|---|---|---|
| `productId?` | `string` |  |
| `bomId?` | `string` |  |
| `status?` | `WorkOrderStatus \| 'inprogress' \| 'partiallycompleted' \| 'canceled' \| 'onhold'` | Work order status; the unseparated spellings (`inprogress`, `onhold`, ...) and `canceled` are accepted too. Types: [`WorkOrderStatus`](#workorderstatus) |
| `priority?` | `WorkOrderPriority` | Work order priority. Types: [`WorkOrderPriority`](#workorderpriority) |
| `assignedTo?` | `string` |  |
| `workCenterId?` | `string` |  |
| `overdueOnly?` | `boolean` |  |
| `limit?` | `number` | Page size (server default 500, hard cap 1000) |
| `offset?` | `number` |  |
| `afterCursor?` | `Array<string>` | Keyset cursor: `[createdAt, id]` |

### WorkOrderOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `workOrderNumber` | `string` |  |
| `productId` | `string` |  |
| `bomId?` | `string` |  |
| `status` | `WorkOrderStatus` | Types: [`WorkOrderStatus`](#workorderstatus) |
| `priority` | `WorkOrderPriority` | Types: [`WorkOrderPriority`](#workorderpriority) |
| `quantityToBuild` | `number` |  |
| `quantityCompleted` | `number` |  |
| `version` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### WorkOrderPriority

Work order priority (`WorkOrderOutput.priority`, `CreateWorkOrderInput.priority`, `WorkOrderFilterInput.priority`).

```ts
type WorkOrderPriority = 'low' | 'normal' | 'high' | 'urgent'
```

One of: `'low'`, `'normal'`, `'high'`, `'urgent'`.

### WorkOrderStatus

Work order status (`WorkOrderOutput.status`, `WorkOrderFilterInput.status`).

```ts
type WorkOrderStatus = 'planned' | 'in_progress' | 'completed' | 'partially_completed' | 'cancelled' | 'on_hold'
```

One of: `'planned'`, `'in_progress'`, `'completed'`, `'partially_completed'`, `'cancelled'`, `'on_hold'`.

### X402A2ASkill

A2A commerce skill as rendered on `X402AgentCardOutput.a2ASkills`.

```ts
type X402A2ASkill = 'commerce.sell' | 'commerce.buy' | 'commerce.quote' | 'commerce.request_quote' | 'commerce.fulfill' | 'commerce.ship' | 'commerce.digital_deliver' | 'commerce.process_return' | 'commerce.refund' | 'commerce.support'
```

One of: `'commerce.sell'`, `'commerce.buy'`, `'commerce.quote'`, `'commerce.request_quote'`, `'commerce.fulfill'`, `'commerce.ship'`, `'commerce.digital_deliver'`, `'commerce.process_return'`, `'commerce.refund'`, `'commerce.support'`.

### X402A2ASkillInput

A2A commerce skill accepted on input: the rendered form or the bare skill name.

```ts
type X402A2ASkillInput = X402A2ASkill | 'sell' | 'buy' | 'quote' | 'request_quote' | 'fulfill' | 'ship' | 'digital_deliver' | 'process_return' | 'refund' | 'support'
```

Types: [`X402A2ASkill`](#x402a2askill)

### X402AgentCardFilterInput

| Field | Type | Description |
|---|---|---|
| `walletAddress?` | `string` |  |
| `trustLevel?` | `X402TrustLevelInput` | Types: [`X402TrustLevelInput`](#x402trustlevelinput) |
| `minTrustLevel?` | `X402TrustLevelInput` | Types: [`X402TrustLevelInput`](#x402trustlevelinput) |
| `network?` | `X402NetworkInput` | Types: [`X402NetworkInput`](#x402networkinput) |
| `asset?` | `X402AssetInput` | Types: [`X402AssetInput`](#x402assetinput) |
| `skill?` | `X402A2ASkillInput` | Types: [`X402A2ASkillInput`](#x402a2askillinput) |
| `active?` | `boolean` |  |
| `merchantId?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### X402AgentCardInput

| Field | Type | Description |
|---|---|---|
| `name` | `string` |  |
| `description?` | `string` |  |
| `walletAddress` | `string` |  |
| `publicKey` | `string` |  |
| `supportedNetworks?` | `X402NetworkInput[]` | Types: [`X402NetworkInput`](#x402networkinput) |
| `supportedAssets?` | `X402AssetInput[]` | Types: [`X402AssetInput`](#x402assetinput) |
| `a2ASkills?` | `X402A2ASkillInput[]` | Types: [`X402A2ASkillInput`](#x402a2askillinput) |
| `trustLevel?` | `X402TrustLevelInput` | Types: [`X402TrustLevelInput`](#x402trustlevelinput) |
| `endpointUrl?` | `string` |  |
| `endpointProtocol?` | `string` |  |
| `merchantId?` | `string` |  |
| `merchantName?` | `string` |  |
| `businessCategory?` | `string` |  |
| `maxTransactionAmount?` | `number` |  |
| `dailyVolumeLimit?` | `number` |  |
| `requiresKyc?` | `boolean` |  |
| `metadata?` | `string` |  |

### X402AgentCardOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `name` | `string` |  |
| `description?` | `string` |  |
| `walletAddress` | `string` |  |
| `publicKey` | `string` |  |
| `supportedNetworks` | `X402Network[]` | Types: [`X402Network`](#x402network) |
| `supportedAssets` | `X402Asset[]` | Types: [`X402Asset`](#x402asset) |
| `a2ASkills` | `X402A2ASkill[]` | Types: [`X402A2ASkill`](#x402a2askill) |
| `trustLevel` | `X402TrustLevel` | Types: [`X402TrustLevel`](#x402trustlevel) |
| `verifiedAt?` | `string` |  |
| `verificationMethod?` | `string` |  |
| `endpointUrl?` | `string` |  |
| `endpointProtocol?` | `string` |  |
| `merchantId?` | `string` |  |
| `merchantName?` | `string` |  |
| `businessCategory?` | `string` |  |
| `maxTransactionAmount?` | `number` |  |
| `dailyVolumeLimit?` | `number` |  |
| `requiresKyc` | `boolean` |  |
| `active` | `boolean` |  |
| `suspendedAt?` | `string` |  |
| `suspensionReason?` | `string` |  |
| `metadata?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### X402Asset

x402 asset as rendered on outputs (lower-cased).

```ts
type X402Asset = 'usdc' | 'tether' | 'ss_usd' | 'wss_usd' | 'dai' | 'ether'
```

One of: `'usdc'`, `'tether'`, `'ss_usd'`, `'wss_usd'`, `'dai'`, `'ether'`.

### X402AssetInput

x402 asset accepted on input: the rendered form or the ticker aliases (case-insensitive).

```ts
type X402AssetInput = X402Asset | 'USDC' | 'USDT' | 'TETHER' | 'usdt' | 'ssUSD' | 'SSUSD' | 'SS_USD' | 'ssusd' | 'wssUSD' | 'WSSUSD' | 'WSS_USD' | 'wssusd' | 'DAI' | 'ETH' | 'ETHER' | 'eth'
```

Types: [`X402Asset`](#x402asset)

### X402CreateIntentInput

| Field | Type | Description |
|---|---|---|
| `payerAddress` | `string` |  |
| `payeeAddress` | `string` |  |
| `amount` | `number` |  |
| `asset?` | `X402AssetInput` | Types: [`X402AssetInput`](#x402assetinput) |
| `network?` | `X402NetworkInput` | Types: [`X402NetworkInput`](#x402networkinput) |
| `signatureScheme?` | `X402SignatureScheme` | Types: [`X402SignatureScheme`](#x402signaturescheme) |
| `nonce?` | `number` |  |
| `validitySeconds?` | `number` |  |
| `resourceUri?` | `string` |  |
| `resourceMethod?` | `string` |  |
| `description?` | `string` |  |
| `cartId?` | `string` |  |
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `merchantId?` | `string` |  |
| `idempotencyKey?` | `string` |  |
| `metadata?` | `string` |  |

### X402CreditAccountOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `payerAddress` | `string` |  |
| `asset` | `X402Asset` | Types: [`X402Asset`](#x402asset) |
| `network` | `X402Network` | Types: [`X402Network`](#x402network) |
| `balance` | `number` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### X402CreditAdjustmentInput

| Field | Type | Description |
|---|---|---|
| `payerAddress` | `string` |  |
| `asset?` | `X402AssetInput` | Types: [`X402AssetInput`](#x402assetinput) |
| `network?` | `X402NetworkInput` | Types: [`X402NetworkInput`](#x402networkinput) |
| `amount` | `number` |  |
| `reason?` | `string` |  |
| `referenceId?` | `string` |  |
| `metadata?` | `string` |  |

### X402CreditBalanceInput

| Field | Type | Description |
|---|---|---|
| `payerAddress` | `string` |  |
| `asset?` | `X402AssetInput` | Types: [`X402AssetInput`](#x402assetinput) |
| `network?` | `X402NetworkInput` | Types: [`X402NetworkInput`](#x402networkinput) |

### X402CreditDirection

x402 credit ledger direction as rendered on `X402CreditTransactionOutput.direction`.

```ts
type X402CreditDirection = 'credit' | 'debit'
```

One of: `'credit'`, `'debit'`.

### X402CreditDirectionInput

x402 credit ledger direction accepted by `X402CreditTransactionFilterInput.direction` (case-insensitive).

```ts
type X402CreditDirectionInput = X402CreditDirection | 'cr' | 'dr'
```

Types: [`X402CreditDirection`](#x402creditdirection)

### X402CreditTransactionFilterInput

| Field | Type | Description |
|---|---|---|
| `payerAddress?` | `string` |  |
| `asset?` | `X402AssetInput` | Types: [`X402AssetInput`](#x402assetinput) |
| `network?` | `X402NetworkInput` | Types: [`X402NetworkInput`](#x402networkinput) |
| `direction?` | `X402CreditDirectionInput` | Types: [`X402CreditDirectionInput`](#x402creditdirectioninput) |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### X402CreditTransactionOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `accountId` | `string` |  |
| `payerAddress` | `string` |  |
| `asset` | `X402Asset` | Types: [`X402Asset`](#x402asset) |
| `network` | `X402Network` | Types: [`X402Network`](#x402network) |
| `direction` | `X402CreditDirection` | Types: [`X402CreditDirection`](#x402creditdirection) |
| `amount` | `number` |  |
| `balanceAfter` | `number` |  |
| `reason?` | `string` |  |
| `referenceId?` | `string` |  |
| `metadata?` | `string` |  |
| `createdAt` | `string` |  |

### X402IntentFilterInput

| Field | Type | Description |
|---|---|---|
| `payerAddress?` | `string` |  |
| `payeeAddress?` | `string` |  |
| `status?` | `X402IntentStatusInput` | Types: [`X402IntentStatusInput`](#x402intentstatusinput) |
| `network?` | `X402NetworkInput` | Types: [`X402NetworkInput`](#x402networkinput) |
| `asset?` | `X402AssetInput` | Types: [`X402AssetInput`](#x402assetinput) |
| `orderId?` | `string` |  |
| `batchId?` | `string` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### X402IntentOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `version` | `string` |  |
| `status` | `X402IntentStatus` | Types: [`X402IntentStatus`](#x402intentstatus) |
| `payerAddress` | `string` |  |
| `payeeAddress` | `string` |  |
| `amount` | `number` |  |
| `amountDecimal` | `number` | **Deprecated.** Use the `amountDecimalExact` twin; float money will be removed in 2.0. |
| `amountDecimalExact` | `string` | Exact base-10 amount decimal, straight from the engine's `Decimal`. Prefer this field for money. _Exact money: a base-10 decimal string; prefer it over any float twin._ |
| `asset` | `X402Asset` | Types: [`X402Asset`](#x402asset) |
| `network` | `X402Network` | Types: [`X402Network`](#x402network) |
| `chainId` | `number` |  |
| `tokenAddress?` | `string` |  |
| `createdAtUnix` | `number` |  |
| `validUntil` | `number` |  |
| `nonce` | `number` |  |
| `idempotencyKey?` | `string` |  |
| `resourceUri?` | `string` |  |
| `resourceMethod?` | `string` |  |
| `description?` | `string` |  |
| `orderId?` | `string` |  |
| `invoiceId?` | `string` |  |
| `merchantId?` | `string` |  |
| `signingHash?` | `string` |  |
| `payerSignatureScheme?` | `X402SignatureScheme` | Types: [`X402SignatureScheme`](#x402signaturescheme) |
| `payerSignature?` | `string` |  |
| `payerPublicKey?` | `string` |  |
| `payerSignatureBundle?` | `X402SignatureBundleOutput` | Types: [`X402SignatureBundleOutput`](#x402signaturebundleoutput) |
| `payerPublicKeyBundle?` | `X402PublicKeyBundleOutput` | Types: [`X402PublicKeyBundleOutput`](#x402publickeybundleoutput) |
| `sequenceNumber?` | `number` |  |
| `sequencedAt?` | `string` |  |
| `batchId?` | `string` |  |
| `batchMerkleRoot?` | `string` |  |
| `inclusionProof?` | `Array<string>` |  |
| `txHash?` | `string` |  |
| `blockNumber?` | `number` |  |
| `gasUsed?` | `number` |  |
| `settledAt?` | `string` |  |
| `metadata?` | `string` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### X402IntentStatus

Payment intent status as rendered on `X402IntentOutput.status`.

```ts
type X402IntentStatus = 'created' | 'signed' | 'sequenced' | 'batched' | 'settled' | 'expired' | 'failed' | 'cancelled'
```

One of: `'created'`, `'signed'`, `'sequenced'`, `'batched'`, `'settled'`, `'expired'`, `'failed'`, `'cancelled'`.

### X402IntentStatusInput

Payment intent status accepted by `X402IntentFilterInput.status` (case-insensitive).

```ts
type X402IntentStatusInput = X402IntentStatus | 'canceled'
```

Types: [`X402IntentStatus`](#x402intentstatus)

### X402Network

x402 network as rendered on outputs.

```ts
type X402Network = 'set_chain' | 'set_chain_testnet' | 'base' | 'arc' | 'arc-testnet' | 'base_sepolia' | 'ethereum' | 'ethereum_sepolia' | 'arbitrum' | 'optimism'
```

One of: `'set_chain'`, `'set_chain_testnet'`, `'base'`, `'arc'`, `'arc-testnet'`, `'base_sepolia'`, `'ethereum'`, `'ethereum_sepolia'`, `'arbitrum'`, `'optimism'`.

### X402NetworkInput

x402 network accepted on input: the rendered form or any alias (case-insensitive).

```ts
type X402NetworkInput = X402Network | 'set' | 'ssc' | 'set_testnet' | 'arc_testnet' | 'eth' | 'mainnet' | 'sepolia' | 'arb' | 'op'
```

Types: [`X402Network`](#x402network)

### X402PublicKeyBundleInput

| Field | Type | Description |
|---|---|---|
| `mlDsa65PublicKey` | `Buffer` |  |

### X402PublicKeyBundleOutput

| Field | Type | Description |
|---|---|---|
| `mlDsa65PublicKey` | `Buffer` |  |

### X402SignIntentInput

| Field | Type | Description |
|---|---|---|
| `intentId?` | `string` |  |
| `signatureScheme?` | `X402SignatureScheme` | Types: [`X402SignatureScheme`](#x402signaturescheme) |
| `signature` | `string` |  |
| `publicKey` | `string` |  |
| `signatureBundle?` | `X402SignatureBundleInput` | Types: [`X402SignatureBundleInput`](#x402signaturebundleinput) |
| `publicKeyBundle?` | `X402PublicKeyBundleInput` | Types: [`X402PublicKeyBundleInput`](#x402publickeybundleinput) |

### X402SignatureBundleInput

| Field | Type | Description |
|---|---|---|
| `mlDsa65Signature` | `Buffer` |  |

### X402SignatureBundleOutput

| Field | Type | Description |
|---|---|---|
| `mlDsa65Signature` | `Buffer` |  |

### X402SignatureScheme

x402 signature scheme, shared by outputs and inputs (case-insensitive on input).

```ts
type X402SignatureScheme = 'ed25519' | 'ml_dsa65' | 'ed25519_ml_dsa65'
```

One of: `'ed25519'`, `'ml_dsa65'`, `'ed25519_ml_dsa65'`.

### X402SigningHashInput

| Field | Type | Description |
|---|---|---|
| `payerAddress` | `string` |  |
| `payeeAddress` | `string` |  |
| `amount` | `number` |  |
| `asset` | `X402AssetInput` | Types: [`X402AssetInput`](#x402assetinput) |
| `network` | `X402NetworkInput` | Types: [`X402NetworkInput`](#x402networkinput) |
| `chainId` | `number` |  |
| `validUntil` | `number` |  |
| `nonce` | `number` |  |
| `resourceUri?` | `string` |  |
| `resourceMethod?` | `string` |  |

### X402TrustLevel

Agent trust level as rendered on `X402AgentCardOutput.trustLevel`.

```ts
type X402TrustLevel = 'sandbox' | 'standard' | 'verified' | 'enterprise'
```

One of: `'sandbox'`, `'standard'`, `'verified'`, `'enterprise'`.

### X402TrustLevelInput

Agent trust level accepted on input: the rendered form or an alias (case-insensitive).

```ts
type X402TrustLevelInput = X402TrustLevel | 'test' | 'default' | 'business'
```

Types: [`X402TrustLevel`](#x402trustlevel)

### ZoneShippingMethodFilterInput

| Field | Type | Description |
|---|---|---|
| `zoneId?` | `string` |  |
| `carrier?` | `string` |  |
| `methodType?` | `ShippingMethodType` | flat, weight_based, price_based, calculated, free Types: [`ShippingMethodType`](#shippingmethodtype) |
| `isActive?` | `boolean` |  |
| `limit?` | `number` |  |
| `offset?` | `number` |  |

### ZoneShippingMethodOutput

| Field | Type | Description |
|---|---|---|
| `id` | `string` |  |
| `zoneId` | `string` |  |
| `name` | `string` |  |
| `carrier?` | `string` |  |
| `methodType` | `ShippingMethodType` | flat, weight_based, price_based, calculated, free Types: [`ShippingMethodType`](#shippingmethodtype) |
| `baseRate` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `minDeliveryDays?` | `number` |  |
| `maxDeliveryDays?` | `number` |  |
| `conditions` | `Array<ShippingConditionOutput>` | Types: [`ShippingConditionOutput`](#shippingconditionoutput) |
| `isActive` | `boolean` |  |
| `createdAt` | `string` |  |
| `updatedAt` | `string` |  |

### ZoneShippingRateOutput

| Field | Type | Description |
|---|---|---|
| `methodId` | `string` |  |
| `methodName` | `string` |  |
| `carrier?` | `string` |  |
| `rate` | `string` | Exact decimal string |
| `currency` | `string` |  |
| `minDeliveryDays?` | `number` |  |
| `maxDeliveryDays?` | `number` |  |

### ZoneShippingRateRequestInput

| Field | Type | Description |
|---|---|---|
| `country` | `string` |  |
| `region?` | `string` |  |
| `postalCode?` | `string` |  |
| `weight?` | `string` | Exact decimal string |
| `orderTotal?` | `string` | Exact decimal string |
| `currency` | `string` | ISO 4217 currency code |
