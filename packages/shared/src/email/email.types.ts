/** Mensaje de correo transaccional (ADR-0003). */
export interface EmailMessage {
  to: string;
  subject: string;
  text: string;
  html?: string;
}

/** Contrato mínimo del proveedor; log en dev/test y SMTP en stage/prod. */
export interface EmailProvider {
  send(message: EmailMessage): Promise<void>;
}

export type EmailMode = 'log' | 'smtp';
