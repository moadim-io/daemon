declare module "cronstrue" {
  export function toString(
    expression: string,
    options?: { throwExceptionOnParseError?: boolean; verbose?: boolean; use24HourTimeFormat?: boolean },
  ): string;
}
