import { apiRequest } from './http';

export const LEGAL_VERSION = '2026-10-25';
export type LegalDocuments = {
  version: string;
  terms: string;
  privacy: string;
  production_days: number;
};
export function getLegalDocuments() {
  return apiRequest<LegalDocuments>('/api/legal/documents');
}
