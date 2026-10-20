/** Preserve typed permission failures across the Rust/renderer boundary. */
export class ProfileScanError extends Error {
  readonly permissionDenied: boolean;

  constructor(message: string, permissionDenied: boolean) {
    super(message);
    this.name = "ProfileScanError";
    this.permissionDenied = permissionDenied;
  }
}

export function profileScanError(error: unknown): Error {
  if (
    error !== null &&
    typeof error === "object" &&
    "message" in error &&
    typeof error.message === "string" &&
    "permission_denied" in error &&
    typeof error.permission_denied === "boolean"
  ) {
    return new ProfileScanError(error.message, error.permission_denied);
  }
  return error instanceof Error ? error : new Error(String(error));
}

export function isProfileAccessDenied(error: unknown): boolean {
  return error instanceof ProfileScanError && error.permissionDenied;
}
