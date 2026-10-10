/** Current demo schema, not verified Anchor account definitions. */
export type MicroUsdc = string;
export interface PropertyListing {
  id: string;
  name: string;
  location: string;
  type: string;
  image: string; // Bundled path or uploaded image data URL.
  description: string;
  price: string; // Decimal USDC per token, up to six decimal places.
  supply: string; // Fixed positive integer total supply.
  sold: number;
  rent: string; // Owner's estimated monthly property rent in decimal USDC.
  allocation: string; // Owner-published investor percentage, 0–100.
  period: string;
  duration: string;
  tenant: string;
  propertyId: string;
  terms: string;
  demo: boolean;
  ownerListing: boolean;
}
export interface InvestorHolding {
  id: string;
  tokens: number;
  invested: MicroUsdc; // Historical contributions, not current token valuation.
}
export interface RentalPeriod {
  id: string;
  property: string;
  label: string;
  amount: MicroUsdc; // Finalized demo entitlement.
  claimed: boolean;
}
export interface ActivityEntry {
  property: string;
  title: string;
  detail: string;
  time: string;
  status: string;
}
export interface InvestorState {
  holdings: InvestorHolding[];
  periods: RentalPeriod[];
  history: ActivityEntry[];
}
export interface PurchaseReceipt {
  id: string; // Local request identifier, never a blockchain signature.
  property: string;
  quantity: number;
  amount: MicroUsdc;
  createdAt: string;
}
export interface DemoSnapshot {
  properties: PropertyListing[];
  investor: InvestorState;
  balance: MicroUsdc;
  purchases: PurchaseReceipt[];
}
