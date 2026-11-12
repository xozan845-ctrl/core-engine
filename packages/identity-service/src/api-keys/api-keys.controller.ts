import {
  Body,
  Controller,
  Delete,
  Get,
  Headers,
  HttpCode,
  Param,
  Post,
  Query,
} from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiHeader,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiTags,
} from '@nestjs/swagger';
import { IsIn, IsInt, IsOptional, IsString, IsUUID, Length, Max, Min } from 'class-validator';
import { ROLES, Rol, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { ApiKey, ApiKeysService, ApiKeyCreada, ROLES_API_KEY } from './api-keys.service';

export class CrearApiKeyRequestDto {
  @ApiProperty({ description: 'Nombre para identificar la integración', minLength: 2, maxLength: 120 })
  @IsString()
  @Length(2, 120)
  nombre: string;

  @ApiProperty({ description: 'Rol efectivo de la key', enum: ROLES_API_KEY })
  @IsIn(ROLES_API_KEY)
  rol: Rol;

  @ApiProperty({ description: 'Empresa (solo admin/super_admin)', required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  empresa_id?: string;

  @ApiProperty({ description: 'Días de vigencia (opcional)', required: false, minimum: 1, maximum: 365 })
  @IsOptional()
  @IsInt()
  @Min(1)
  @Max(365)
  expira_en_dias?: number;
}

export class ApiKeyResponseDto implements Omit<ApiKey, 'hash'> {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty() nombre: string;
  @ApiProperty() prefijo: string;
  @ApiProperty({ enum: ROLES_API_KEY }) rol: Rol;
  @ApiProperty({ enum: ['activa', 'revocada'] }) estado: 'activa' | 'revocada';
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) expira_en: string | null;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) ultimo_uso_en: string | null;
}

export class ApiKeyCreadaResponseDto extends ApiKeyResponseDto implements ApiKeyCreada {
  @ApiProperty({ description: 'Clave completa; solo se muestra esta vez' }) key: string;
}

@ApiTags('API keys')
@ApiBearerAuth()
@Controller('api/v2/api-keys')
export class ApiKeysController {
  constructor(private readonly apiKeys: ApiKeysService) {}

  @Get()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar API keys de una empresa' })
  @ApiOkResponse({ type: [ApiKeyResponseDto] })
  listar(
    @UsuarioActual() actor: UsuarioContexto,
    @Query('empresa_id') empresaId?: string,
  ): Promise<ApiKey[]> {
    return this.apiKeys.listar(actor, empresaId);
  }

  @Post()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Crear una API key (el secreto se muestra una vez)' })
  @ApiCreatedResponse({ type: ApiKeyCreadaResponseDto })
  crear(
    @UsuarioActual() actor: UsuarioContexto,
    @Body() dto: CrearApiKeyRequestDto,
  ): Promise<ApiKeyCreada> {
    return this.apiKeys.crear(actor, dto);
  }

  @Delete(':id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Revocar una API key' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ schema: { type: 'object', properties: { id: { type: 'string' }, estado: { type: 'string' } } } })
  revocar(
    @UsuarioActual() actor: UsuarioContexto,
    @Param('id') id: string,
  ): Promise<{ id: string; estado: string }> {
    return this.apiKeys.revocar(actor, id);
  }
}

/**
 * Intercambio de una API key por JWT de acceso corto (R-GW-8). Sin refresh:
 * el cliente vuelve a intercambiar cuando caduque.
 */
@ApiTags('API keys')
@Controller('api/v2/auth')
export class TokenApiKeyController {
  constructor(private readonly apiKeys: ApiKeysService) {}

  @Post('token')
  @HttpCode(200)
  @ApiOperation({ summary: 'Intercambiar una API key por un JWT corto' })
  @ApiHeader({ name: 'x-api-key', required: false, description: 'API key de integración' })
  @ApiOkResponse({
    schema: {
      type: 'object',
      required: ['access_token', 'token_type', 'expires_in'],
      properties: {
        access_token: { type: 'string' },
        token_type: { type: 'string', example: 'Bearer' },
        expires_in: { type: 'integer', example: 900 },
      },
    },
  })
  token(
    @Headers('x-api-key') claveCabecera?: string,
    @Body() body?: { api_key?: string },
  ) {
    return this.apiKeys.token(claveCabecera?.trim() || body?.api_key?.trim());
  }
}
