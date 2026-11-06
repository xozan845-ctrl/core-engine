import { Module } from '@nestjs/common';
import { PgModule } from '@core/shared';
import { ItemsController } from './items.controller';
import { ItemsService } from './items.service';

@Module({ imports: [PgModule], controllers: [ItemsController], providers: [ItemsService], exports: [ItemsService] })
export class ItemsModule {}
