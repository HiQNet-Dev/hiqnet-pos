/** Borradores compartidos por los editores de modificadores. */

export interface GroupDraft {
  name: string;
  min: number;
  max: number;
}

export interface OptionDraft {
  name: string;
  kind: "choice" | "extra" | "omission";
  price_delta: string;
}
