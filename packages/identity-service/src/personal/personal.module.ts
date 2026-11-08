import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { PersonalController, PersonalPublicController } from './personal.controller';
import { PersonalService } from './personal.service';

@Module({
  imports: [AuthModule],
  controllers: [PersonalController, PersonalPublicController],
  providers: [PersonalService],
  exports: [PersonalService],
})
export class PersonalModule {}
