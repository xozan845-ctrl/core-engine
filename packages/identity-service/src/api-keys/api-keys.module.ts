import { Module } from '@nestjs/common';
import { ApiKeysController, TokenApiKeyController } from './api-keys.controller';
import { ApiKeysService } from './api-keys.service';

@Module({
  controllers: [ApiKeysController, TokenApiKeyController],
  providers: [ApiKeysService],
  exports: [ApiKeysService],
})
export class ApiKeysModule {}
