export type ParseMode = "auto" | "lattice" | "stream" | "dl";
export type DlFallbackMode = "auto" | "lattice" | "stream";

export interface Table {
  page: number;
  table_index: number;
  headers: string[];
  rows: string[][];
}

export interface ExtractOptions {
  pages?: number[] | string;
  mode?: ParseMode;
  dlModel?: string;
  dlMinConfidence?: number;
  dlFallback?: DlFallbackMode;
  binPath?: string;
  timeoutMs?: number;
  env?: NodeJS.ProcessEnv;
  eventLog?: string;
  eventDocumentKey?: string;
  eventTenantId?: string;
  eventRequestId?: string;
  eventFeedbackTag?: string;
  eventTrainingOptIn?: boolean;
}

export declare function extract(pdfPath: string, options?: ExtractOptions): Promise<Table[]>;

export declare function extractCsv(pdfPath: string, options?: ExtractOptions): Promise<string>;

export declare function extractFromBuffer(
  buffer: Buffer,
  options?: ExtractOptions,
): Promise<Table[]>;

export declare function extractCsvFromBuffer(
  buffer: Buffer,
  options?: ExtractOptions,
): Promise<string>;
