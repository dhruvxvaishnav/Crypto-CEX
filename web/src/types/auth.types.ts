export interface AuthTokens {
  accessToken: string;
  refreshToken: string;
  tokenType: string;
  expiresIn: number;
}

export interface MfaRequired {
  mfaRequired: true;
  mfaToken: string;
}

export type LoginResponse = AuthTokens | MfaRequired;

export function isMfaRequired(r: LoginResponse): r is MfaRequired {
  return "mfaRequired" in r && r.mfaRequired === true;
}

export interface AuthUser {
  id: string;
  email: string;
  totpEnabled: boolean;
}

export interface AuthState {
  user: AuthUser | null;
  accessToken: string | null;
}
